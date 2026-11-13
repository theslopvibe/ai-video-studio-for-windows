param([string]$ConfigPath = "", [string]$AppExe = "")
# Connects ZCode (Z.ai, GLM) to AI Video Studio For Windows: adds mcp.servers."ai-video-studio" -> app.exe --mcp
# to ~/.zcode/cli/config.json (documented at zcode.z.ai/en/docs/mcp-services) and keeps a backup. No Node.js needed.
$ErrorActionPreference = "Stop"
try {
  if (-not $AppExe) { $AppExe = Join-Path $env:LOCALAPPDATA "AI Video Studio For Windows\app.exe" }
  if (-not (Test-Path $AppExe)) { throw "Den vrethike to programma: $AppExe. Egkatesta prota to AI Video Studio For Windows (setup.exe)." }
  $cfg = $ConfigPath
  if (-not $cfg) { $cfg = Join-Path $env:USERPROFILE ".zcode\cli\config.json" }
  $dir = Split-Path $cfg -Parent
  if (-not (Test-Path $dir)) { New-Item -ItemType Directory -Path $dir -Force | Out-Null }
  $utf8 = New-Object System.Text.UTF8Encoding($false)   # no BOM
  $obj = $null
  if (Test-Path $cfg) {
    $raw = [System.IO.File]::ReadAllText($cfg)
    if ($raw.Trim().Length -gt 0) {
      try { $obj = $raw | ConvertFrom-Json } catch { throw "To arxeio rythmiseon $cfg den einai egkyro JSON. Den to allaxa." }
    }
    Copy-Item $cfg ($cfg + ".backup-" + (Get-Date -Format "yyyyMMdd-HHmmss")) -Force
  }
  if ($null -eq $obj) { $obj = New-Object psobject }
  if (-not ($obj.PSObject.Properties.Name -contains "mcp")) { $obj | Add-Member -NotePropertyName "mcp" -NotePropertyValue (New-Object psobject) }
  if (-not ($obj.mcp.PSObject.Properties.Name -contains "servers")) { $obj.mcp | Add-Member -NotePropertyName "servers" -NotePropertyValue (New-Object psobject) }
  $entry = [pscustomobject]@{ type = "stdio"; command = $AppExe; args = @("--mcp") }
  if ($obj.mcp.servers.PSObject.Properties.Name -contains "ai-video-studio") { $obj.mcp.servers."ai-video-studio" = $entry }
  else { $obj.mcp.servers | Add-Member -NotePropertyName "ai-video-studio" -NotePropertyValue $entry }
  [System.IO.File]::WriteAllText($cfg, ($obj | ConvertTo-Json -Depth 20), $utf8)
  Write-Host "OK: $cfg"
  Write-Host ""
  Write-Host "Etoimo! Twra: 1) Kleise kai anoikse ksana to ZCode. 2) Anoikse to AI Video Studio kai sto Settings tsekare to 'Allow the Claude Desktop connection' (einai to idio topiko API)."
}
catch {
  Write-Host ""
  Write-Host "SFALMA: $($_.Exception.Message)" -ForegroundColor Red
}
