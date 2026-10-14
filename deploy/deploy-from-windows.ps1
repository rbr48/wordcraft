<#
.SYNOPSIS
  One-command deployment of the Izhaan Intellect Office Portal & WordCraft Web to VPS from Windows.
.DESCRIPTION
  Uploads the deploy directory to the VPS and executes deploy-vps.sh remotely.
.PARAMETER VpsHost
  The VPS IP or hostname (default: 148.135.137.245).
.PARAMETER VpsUser
  The SSH username (default: root).
.EXAMPLE
  .\deploy\deploy-from-windows.ps1 -VpsHost "148.135.137.245" -VpsUser "root"
#>

[CmdletBinding()]
param(
    [string]$VpsHost = "148.135.137.245",
    [int]$VpsPort = 2222,
    [string]$VpsUser = "root",
    [string]$IdentityFile = "$HOME\.ssh\id_rsa_dbm"
)

$ErrorActionPreference = "Stop"

Write-Host "===================================================================" -ForegroundColor Cyan
Write-Host "  Izhaan Intellect Office Suite · Windows Remote Deployment" -ForegroundColor Cyan
Write-Host "  Target VPS: $VpsUser@$VpsHost (Port $VpsPort)" -ForegroundColor Cyan
Write-Host "  Domain:     office.izhaanintellect.fun" -ForegroundColor Cyan
Write-Host "  SSH Key:    $IdentityFile" -ForegroundColor Cyan
Write-Host "===================================================================" -ForegroundColor Cyan

$DeployDir = "$PSScriptRoot"
$RemoteDir = "/tmp/office-deploy"

$KeyOpt = if (Test-Path $IdentityFile) { @("-i", $IdentityFile) } else { @() }

Write-Host "`n[1/3] Uploading deployment package to VPS ($RemoteDir)..." -ForegroundColor Yellow
# Using scp with legacy protocol (-O) and custom port
scp -O -P $VpsPort @KeyOpt -r "$DeployDir" "${VpsUser}@${VpsHost}:${RemoteDir}"

Write-Host "`n[2/3] Setting permissions on VPS..." -ForegroundColor Yellow
ssh -p $VpsPort @KeyOpt "${VpsUser}@${VpsHost}" "chmod +x ${RemoteDir}/deploy-vps.sh"

Write-Host "`n[3/3] Executing automated deployment on VPS..." -ForegroundColor Yellow
ssh -p $VpsPort @KeyOpt -t "${VpsUser}@${VpsHost}" "bash ${RemoteDir}/deploy-vps.sh"

Write-Host "`n===================================================================" -ForegroundColor Green
Write-Host "  Deployment finished!" -ForegroundColor Green
Write-Host "  Visit: https://office.izhaanintellect.fun/" -ForegroundColor Green
Write-Host "===================================================================" -ForegroundColor Green
