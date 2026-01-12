param(
    [switch]$NoDebugHeap
)

if (-not (Get-Command WinDbgX -ErrorAction SilentlyContinue)) {
    Write-Error "WinDbgX command not found. Please ensure WinDbg Preview is installed and added to your PATH."
    exit 1
}

$scriptPath = $PSScriptRoot
$envJsonPath = Join-Path $scriptPath "..\env.json"

if (-not (Test-Path $envJsonPath)) {
    Write-Error "env.json not found at '$envJsonPath'. Please ensure it exists."
    exit 1
}

$envConfig = Get-Content $envJsonPath | ConvertFrom-Json

$dllDeployDirectory = Resolve-Path $envConfig.dll_deploy_directory

Write-Host "Launching WinDbgX with DLL path: $dllDeployDirectory"
$me3LaunchArgs = @("launch", "-g", "er", "--disable-arxan", "--savefile", "`"DEV.sl2`"", "--native", "eldenring_remapper.dll")
$me3Path = Resolve-Path (Join-Path $scriptPath "me3\me3.exe")

$windbgLaunchArgs = @("/o", "/g", "/G", "/y", "`"$dllDeployDirectory`"", "`"$me3Path`"")
if ($NoDebugHeap) {
    Write-Host "User specified no debug heap launch"
    $windbgLaunchArgs = @("/hd") + $windbgLaunchArgs
}
Start-Process -FilePath "WinDbgX" -ArgumentList ($windbgLaunchArgs + $me3LaunchArgs) -WorkingDirectory $dllDeployDirectory