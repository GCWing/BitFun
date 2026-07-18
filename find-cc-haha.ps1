$ErrorActionPreference = 'Stop'
$drives = @('E:\')
foreach ($drive in $drives) {
    Write-Host "Searching in $drive..."
    $results = Get-ChildItem -Path $drive -Directory -Depth 2 -ErrorAction SilentlyContinue | Where-Object { $_.Name -match 'cc-haha|claude.code|CC-haha' }
    foreach ($r in $results) {
        Write-Host "FOUND: $($r.FullName)"
    }
}
Write-Host "Search complete."
