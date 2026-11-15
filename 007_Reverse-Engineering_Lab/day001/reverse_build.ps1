$ErrorActionPreference = "Stop"

# --------------------------------------------------
# 1. Paths
# --------------------------------------------------

$ProjectDir = $PSScriptRoot
$LabDir = Join-Path $ProjectDir "gdi32_lab"

# 64-bit Windows system DLL
$SourceDll = Join-Path $env:WINDIR "System32\gdi32.dll"

# Copy used for reverse engineering
$Dll = Join-Path $LabDir "gdi32.dll"

New-Item -ItemType Directory -Force -Path $LabDir |
    Out-Null

if (-not (Test-Path -LiteralPath $SourceDll)) {
    throw "Source DLL not found: $SourceDll"
}

Copy-Item -LiteralPath $SourceDll -Destination $Dll -Force

Write-Host "Source DLL: $SourceDll"
Write-Host "Lab DLL:    $Dll"

# --------------------------------------------------
# 2. Windows information
# --------------------------------------------------

Get-ComputerInfo |
    Select-Object WindowsProductName,
                  WindowsVersion,
                  OsBuildNumber,
                  OsArchitecture |
    Out-File (Join-Path $LabDir "windows.txt")

# --------------------------------------------------
# 3. File information
# --------------------------------------------------

Get-Item -LiteralPath $Dll |
    Format-List * |
    Out-File (Join-Path $LabDir "file_info.txt")

# --------------------------------------------------
# 4. SHA-256
# --------------------------------------------------

Get-FileHash -LiteralPath $Dll -Algorithm SHA256 |
    Format-List |
    Out-File (Join-Path $LabDir "hash.txt")

# --------------------------------------------------
# 5. Find PE inspection tool
# --------------------------------------------------

$LLVM = Get-Command llvm-readobj.exe -ErrorAction SilentlyContinue
$DUMPBIN = Get-Command dumpbin.exe -ErrorAction SilentlyContinue

if ($LLVM) {

    Write-Host "Using llvm-readobj: $($LLVM.Source)"

    $Tool = $LLVM.Source

    # PE headers
    & $Tool --file-headers $Dll 2>&1 |
        Out-File (Join-Path $LabDir "headers.txt")

    # PE sections
    & $Tool --sections $Dll 2>&1 |
        Out-File (Join-Path $LabDir "sections.txt")

    # Export table
    & $Tool --coff-exports $Dll 2>&1 |
        Out-File (Join-Path $LabDir "exports.txt")

    # Import table
    & $Tool --coff-imports $Dll 2>&1 |
        Out-File (Join-Path $LabDir "imports.txt")

    if ($LASTEXITCODE -ne 0) {
        throw "llvm-readobj failed."
    }

}
elseif ($DUMPBIN) {

    Write-Host "Using dumpbin: $($DUMPBIN.Source)"

    $Tool = $DUMPBIN.Source

    # PE headers (includes section headers)
    & $Tool /headers $Dll 2>&1 |
        Out-File (Join-Path $LabDir "headers.txt")

    if ($LASTEXITCODE -ne 0) {
        throw "dumpbin /headers failed."
    }

    # dumpbin /headers already contains section information.
    "Section headers are included in headers.txt (DUMPBIN /HEADERS)." |
        Out-File (Join-Path $LabDir "sections.txt")

    # Export table
    & $Tool /exports $Dll 2>&1 |
        Out-File (Join-Path $LabDir "exports.txt")

    if ($LASTEXITCODE -ne 0) {
        throw "dumpbin /exports failed."
    }

    # Import table
    & $Tool /imports $Dll 2>&1 |
        Out-File (Join-Path $LabDir "imports.txt")

    if ($LASTEXITCODE -ne 0) {
        throw "dumpbin /imports failed."
    }

}
else {

    Write-Warning "Neither llvm-readobj nor dumpbin was found."

    Write-Host ""
    Write-Host "Install LLVM or Visual Studio C++ Build Tools."
    Write-Host "Metadata and SHA-256 have been collected,"
    Write-Host "but PE inspection was not completed."

    exit 2
}

# --------------------------------------------------
# 6. Completed
# --------------------------------------------------

Write-Host ""
Write-Host "Day 1 inspection completed successfully."
Write-Host "Output directory: $LabDir"

