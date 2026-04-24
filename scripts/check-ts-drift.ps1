# scripts/check-ts-drift.ps1
# §5.7 ts-rs drift 校验
# 运行 ts-rs 导出后检查 git diff，漂移即失败

$ErrorActionPreference = "Stop"

Write-Host "Running ts-rs export..."
cargo test -p winvibe-app --features ts-export 2>&1

Write-Host "Checking for drift in generated types..."
$diff = git diff --exit-code web/src/types/generated/ 2>&1

if ($LASTEXITCODE -ne 0) {
    Write-Host "ERROR: ts-rs generated types have drifted!" -ForegroundColor Red
    Write-Host "Run 'cargo test -p winvibe-app --features ts-export' locally and commit the changes." -ForegroundColor Yellow
    Write-Host $diff
    exit 1
}

Write-Host "ts-rs types are up to date." -ForegroundColor Green
exit 0
