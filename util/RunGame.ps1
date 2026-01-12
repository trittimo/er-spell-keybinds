$scriptPath = $PSScriptRoot
$envJsonPath = Join-Path $scriptPath "..\env.json"

if (-not (Test-Path $envJsonPath)) {
    Write-Error "env.json not found at '$envJsonPath'. Please ensure it exists."
    exit 1
}

$envConfig = Get-Content $envJsonPath | ConvertFrom-Json

$dllDeployDirectory = Resolve-Path $envConfig.dll_deploy_directory

$me3LaunchArgs = @("launch", "-g", "er", "--disable-arxan", "--savefile", "`"DEV.sl2`"", "--native", "eldenring_remapper.dll")
$me3Path = Resolve-Path (Join-Path $scriptPath "me3\me3.exe")

Write-Host "Starting $me3Path"
Start-Process $me3Path -ArgumentList $me3LaunchArgs -WorkingDirectory $dllDeployDirectory