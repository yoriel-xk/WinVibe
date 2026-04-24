# scripts/check-deps.ps1
# §2.4 CI 依赖矩阵校验
# 通过 cargo metadata 解析 packages 与 dependencies，
# 对每个 winvibe-* crate 检查其 deps 是否仅在允许列表内。

$ErrorActionPreference = "Stop"

$metadata = cargo metadata --format-version 1 --no-deps | ConvertFrom-Json

# 允许的内部依赖矩阵
$allowed = @{
    "winvibe-core"           = @()
    "winvibe-hook-server"    = @("winvibe-core")
    "winvibe-hookcli"        = @("winvibe-core")
    "winvibe-app"            = @("winvibe-core", "winvibe-hook-server")
    "winvibe-contract-tests" = @("winvibe-core", "winvibe-hook-server", "winvibe-hookcli")
    "winvibe-e2e"            = @("winvibe-core", "winvibe-hook-server", "winvibe-hookcli", "winvibe-app")
}

# 禁止的三方依赖
$forbidden = @{
    "winvibe-core"        = @("tokio", "axum", "tauri", "ureq", "toml")
    "winvibe-hook-server" = @("tauri")
    "winvibe-hookcli"     = @("tokio", "hyper", "reqwest")
}

$exitCode = 0

foreach ($pkg in $metadata.packages) {
    if (-not $pkg.name.StartsWith("winvibe-")) { continue }

    $deps = $pkg.dependencies | Where-Object { $_.name.StartsWith("winvibe-") } | ForEach-Object { $_.name }
    $allowedDeps = $allowed[$pkg.name]

    foreach ($dep in $deps) {
        if ($allowedDeps -notcontains $dep) {
            Write-Host "ERROR: $($pkg.name) depends on $dep (not allowed)" -ForegroundColor Red
            $exitCode = 1
        }
    }

    # 检查禁止的三方依赖
    if ($forbidden.ContainsKey($pkg.name)) {
        $allDeps = $pkg.dependencies | ForEach-Object { $_.name }
        foreach ($banned in $forbidden[$pkg.name]) {
            if ($allDeps -contains $banned) {
                Write-Host "ERROR: $($pkg.name) depends on forbidden crate $banned" -ForegroundColor Red
                $exitCode = 1
            }
        }
    }
}

if ($exitCode -eq 0) {
    Write-Host "Dependency matrix check passed." -ForegroundColor Green
}

exit $exitCode
