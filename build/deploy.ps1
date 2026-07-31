param(
    [switch]$SkipBuild,
    [switch]$NoDeploy
)

if (-not $SkipBuild) {
    Write-Host "Building release..." -ForegroundColor Cyan
    cargo build --release
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    Copy-Item -Force target/release/crd.exe build/crd.exe
    Write-Host "Build complete: build/crd.exe" -ForegroundColor Green
}

if (-not $NoDeploy) {
    if (-not (Test-Path "build/crd.exe")) {
        Write-Host "Error: build/crd.exe not found. Run without -SkipBuild or build first." -ForegroundColor Red
        exit 1
    }
    $null = New-Item -ItemType Directory -Force -Path "C:\Tools"
    Copy-Item -Force build/crd.exe C:\Tools/crd.exe
    Write-Host "Deployed to C:\Tools\crd.exe" -ForegroundColor Green
}
