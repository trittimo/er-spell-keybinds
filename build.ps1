$CONFIG = Get-Content -Path "env.json" -Raw | ConvertFrom-Json
$MOD_DIRECTORY = $ExecutionContext.InvokeCommand.ExpandString($CONFIG.MOD_DIRECTORY)
cargo build --release
if ($LASTEXITCODE -eq 0) {
    Copy-Item -Path ".\target\release\spell_keybinds.dll" -Destination $MOD_DIRECTORY -Force
    Copy-Item -Path ".\spell_keybinds.ini" -Destination $MOD_DIRECTORY -Force
    Write-Host "Copied dll and ini file to $MOD_DIRECTORY"
}