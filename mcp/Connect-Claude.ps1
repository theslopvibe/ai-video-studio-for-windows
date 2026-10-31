param([string]$ConfigPath = "", [string]$AppExe = "")
# Connects Claude Desktop to AI Video Studio For Windows without any extension file and without Node.js.
# It adds one entry ("ai-video-studio" -> app.exe --mcp) to claude_desktop_config.json and keeps a backup.
$ErrorActionPreference = "Stop"
try {
  if (-not $AppExe) { $AppExe = Join-Path $env:LOCALAPPDATA "AI Video Studio For Windows\app.exe" }
  if (-not (Test-Path $AppExe)) { throw "Den vrethike to programma: $AppExe. Egkatesta prota to AI Video Studio For Windows (setup.exe)." }

  $targets = @()
  if ($ConfigPath) { $targets += $ConfigPath }
  else {
    $std = Join-Path $env:APPDATA "Claude\claude_desktop_config.json"
    $targets += $std
    $pk = Join-Path $env:LOCALAPPDATA "Packages"
    if (Test-Path $pk) {
      Get-ChildItem $pk -Directory -Filter "Claude_*" -ErrorAction SilentlyContinue | ForEach-Object {
        $targets += (Join-Path $_.FullName "LocalCache\Roaming\Claude\claude_desktop_config.json")
      }
    }
    # use only the files that exist; if none exists, create the standard one
    $existing = @($targets | Where-Object { Test-Path $_ })
    if ($existing.Count -gt 0) { $targets = $existing } else { $targets = @($std) }
  }

  $utf8 = New-Object System.Text.UTF8Encoding($false)   # no BOM: a BOM breaks JSON readers
  foreach ($cfg in $targets) {
    $dir = Split-Path $cfg -Parent
    if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
    $obj = $null
    if (Test-Path $cfg) {
      $raw = [System.IO.File]::ReadAllText($cfg)
      if ($raw.Trim().Length -gt 0) {
        try { $obj = $raw | ConvertFrom-Json } catch { throw "To arxeio rythmiseon $cfg den einai egkyro JSON. Den to allaxa." }
      }
      Copy-Item $cfg ($cfg + ".backup-" + (Get-Date -Format "yyyyMMdd-HHmmss")) -Force
    }
    if ($null -eq $obj) { $obj = New-Object psobject }
    if (-not ($obj.PSObject.Properties.Name -contains "mcpServers")) {
      $obj | Add-Member -NotePropertyName "mcpServers" -NotePropertyValue (New-Object psobject)
    }
    $entry = [pscustomobject]@{ command = $AppExe; args = @("--mcp") }
    if ($obj.mcpServers.PSObject.Properties.Name -contains "ai-video-studio") { $obj.mcpServers."ai-video-studio" = $entry }
    else { $obj.mcpServers | Add-Member -NotePropertyName "ai-video-studio" -NotePropertyValue $entry }
    [System.IO.File]::WriteAllText($cfg, ($obj | ConvertTo-Json -Depth 20), $utf8)
    Write-Host "OK: $cfg"
  }
  Write-Host ""
  Write-Host "Etoimo! Twra: 1) Kleise ELEFTHERA to Claude Desktop (deksi klik sto eikonidio dipla sto roloi -> Quit)."
  Write-Host "             2) Anoikse to ksana. 3) Sto programma: Settings -> Claude Desktop connection -> tsekare to Allow."
}
catch {
  Write-Host ""
  Write-Host "SFALMA: $($_.Exception.Message)" -ForegroundColor Red
}
