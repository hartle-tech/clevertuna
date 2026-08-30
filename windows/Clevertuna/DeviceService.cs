using System.Diagnostics;
using System.Text.Json;

namespace Clevertuna;

/// <summary>
/// Everything the app needs from the keyboard, behind one seam — the same seam
/// the macOS app uses, fulfilled the same way: the Rust core as a subprocess.
///
/// The core is the reference implementation of a reverse-engineered protocol.
/// Reimplementing it per platform would mean rediscovering the same hardware
/// truths three times, and getting them subtly wrong twice.
/// </summary>
public interface IDeviceService
{
    Task<LookModel> LookAsync(bool random = false, int? seed = null);
    Task ApplyAsync(LookModel model);
    Task<string> PreviewAsync(string path);
}

public sealed class RustCoreBackend : IDeviceService
{
    private readonly string _binary;
    // The keyboard grants one connection, so calls are serialised.
    private readonly SemaphoreSlim _gate = new(1, 1);

    public RustCoreBackend(string? binary = null)
    {
        _binary = binary ?? Path.Combine(AppContext.BaseDirectory,
            OperatingSystem.IsWindows() ? "clevertuna-core.exe" : "clevertuna-core");
    }

    private async Task<string> RunAsync(params string[] args)
    {
        await _gate.WaitAsync();
        try
        {
            var psi = new ProcessStartInfo(_binary)
            {
                RedirectStandardOutput = true,
                RedirectStandardError = true,
            };
            psi.ArgumentList.Add("--no-color");
            foreach (var a in args) psi.ArgumentList.Add(a);

            using var p = Process.Start(psi)
                          ?? throw new InvalidOperationException($"could not start {_binary}");
            // Read before waiting: a full pipe buffer would deadlock the child.
            var stdout = await p.StandardOutput.ReadToEndAsync();
            var stderr = await p.StandardError.ReadToEndAsync();
            await p.WaitForExitAsync();
            if (p.ExitCode != 0)
                throw new InvalidOperationException(
                    string.IsNullOrWhiteSpace(stderr) ? stdout : stderr);
            return stdout;
        }
        finally { _gate.Release(); }
    }

    public async Task<LookModel> LookAsync(bool random = false, int? seed = null)
    {
        var args = new List<string> { "look" };
        if (random) args.Add("random");
        if (seed is int s) { args.Add("--seed"); args.Add(s.ToString()); }
        var json = await RunAsync(args.ToArray());
        return JsonSerializer.Deserialize<LookModel>(json)
               ?? throw new InvalidDataException("the core printed a look this app cannot read");
    }

    public async Task ApplyAsync(LookModel model)
    {
        // The core owns the arithmetic to the device's numbers, so the model
        // goes back exactly the way it came.
        var path = Path.Combine(Path.GetTempPath(), $"clevertuna-look-{Guid.NewGuid():N}.json");
        try
        {
            await File.WriteAllTextAsync(path, JsonSerializer.Serialize(model));
            await RunAsync("look", "apply", path);
        }
        finally { if (File.Exists(path)) File.Delete(path); }
    }

    public Task<string> PreviewAsync(string path) => RunAsync("look", "preview", path);
}
