# scripts/check-tracing.ps1
# 检查 Span::current() 禁用（§4.3）
# 检查 fail-open 残留

$ErrorActionPreference = "Stop"
$exitCode = 0

# 禁止 Span::current()
$spanCurrentFiles = Get-ChildItem -Path crates -Recurse -Filter "*.rs" |
    Select-String -Pattern "Span::current\(\)" -List |
    ForEach-Object { $_.Path }
if ($spanCurrentFiles) {
    Write-Host "ERROR: Span::current() found (§4.3 禁止):" -ForegroundColor Red
    $spanCurrentFiles | ForEach-Object { Write-Host "  $_" }
    $exitCode = 1
}

# 禁止 fail-open 残留
$failOpenFiles = Get-ChildItem -Path crates -Recurse -Filter "*.rs" |
    Select-String -Pattern "fail.open" -List |
    ForEach-Object { $_.Path }
if ($failOpenFiles) {
    Write-Host "ERROR: fail-open reference found (已废弃):" -ForegroundColor Red
    $failOpenFiles | ForEach-Object { Write-Host "  $_" }
    $exitCode = 1
}

if ($exitCode -eq 0) {
    Write-Host "Tracing convention check passed." -ForegroundColor Green
}

exit $exitCode
