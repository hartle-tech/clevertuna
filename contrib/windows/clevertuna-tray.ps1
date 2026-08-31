# Clevertuna in the Windows notification area.
#
# A real tray icon without shipping a GUI framework with the binary: Windows
# already has WinForms, so the tray lives here and every entry calls the same
# executable the CLI does. The menu itself comes from
# `clevertuna menu --format json`, so this file never has to know what the
# actions are.
#
#   powershell -ExecutionPolicy Bypass -File clevertuna-tray.ps1
#
# To start it with Windows, put a shortcut to that command in  shell:startup

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$exe = if ($env:CLEVERTUNA) { $env:CLEVERTUNA } else { "clevertuna.exe" }

function Invoke-Clevertuna {
    param([string[]] $Arguments)
    try {
        return (& $exe @Arguments 2>&1 | Out-String).Trim()
    } catch {
        return "failed: $_"
    }
}

$icon = New-Object System.Windows.Forms.NotifyIcon
$icon.Icon = [System.Drawing.SystemIcons]::Information
$icon.Visible = $true
$icon.Text = "Clevertuna"

function Show-Result {
    param([string] $Title, [string] $Body)
    $icon.BalloonTipTitle = $Title
    $icon.BalloonTipText  = $Body
    $icon.ShowBalloonTip(4000)
}

function Build-Menu {
    $menu = New-Object System.Windows.Forms.ContextMenuStrip
    $model = $null
    try { $model = Invoke-Clevertuna @("menu", "--format", "json") | ConvertFrom-Json } catch { }

    if ($null -eq $model) {
        $missing = $menu.Items.Add("clevertuna not found")
        $missing.Enabled = $false
        return $menu
    }

    foreach ($item in $model.items) {
        if ($item.id -eq "status") {
            $state = $menu.Items.Add($item.label)
            $state.Enabled = $false
            [void] $menu.Items.Add("-")
            continue
        }

        $entry  = $menu.Items.Add($item.label)
        $id     = $item.id

        # No confirmation: everything here changes the lighting, which the next
        # click undoes, and a picker that asks whether you meant it is not a
        # picker. The balloon reports what happened instead.
        $entry.Add_Click({
            $result = Invoke-Clevertuna @("--no-color", "do", $id)
            Show-Result "Clevertuna" (($result -split "`n") | Select-Object -Last 1)
        }.GetNewClosure())
    }

    [void] $menu.Items.Add("-")
    $quit = $menu.Items.Add("Quit")
    $quit.Add_Click({
        $icon.Visible = $false
        [System.Windows.Forms.Application]::Exit()
    })
    return $menu
}

# Rebuild on every click so the gallery and the connection state are current.
$icon.Add_MouseUp({
    $icon.ContextMenuStrip = Build-Menu
    $icon.ContextMenuStrip.Show([System.Windows.Forms.Cursor]::Position)
})

[System.Windows.Forms.Application]::Run()
