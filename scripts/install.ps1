$Arch = if ($env:PROCESSOR_ARCHITECTURE -eq "ARM64") { "arm64" } else { "x64" }

$Repo = "Varenik-vkusny/blackbox"
$Asset = "blackbox-windows-${Arch}.zip"
$Url = "https://github.com/${Repo}/releases/latest/download/${Asset}"

Write-Host "Downloading BlackBox..."
$TempZip = "$env:TEMP\blackbox.zip"
Invoke-WebRequest -Uri $Url -OutFile $TempZip -UseBasicParsing

$InstallDir = "$env:LOCALAPPDATA\BlackBox\bin"
New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
Expand-Archive -Path $TempZip -DestinationPath $InstallDir -Force

$CurrentPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($CurrentPath -notlike "*$InstallDir*") {
  [Environment]::SetEnvironmentVariable("Path", "$CurrentPath;$InstallDir", "User")
  Write-Host "Added $InstallDir to PATH. Restart your terminal."
}

& "$InstallDir\blackbox.exe" setup --auto
