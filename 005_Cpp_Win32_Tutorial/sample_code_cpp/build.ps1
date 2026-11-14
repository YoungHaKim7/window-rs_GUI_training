param(
    [string]$cmd,
    [string]$arg
)

# -----------------------------
# OS detect
# -----------------------------

$os = $env:OS

# -----------------------------
# project name
# -----------------------------

$project_name = Split-Path (Get-Location) -Leaf
$full_project_name = Get-Location

# -----------------------------
# compiler detect (Windows)
# -----------------------------

function find-gcc {

    $c = Get-Command gcc -ErrorAction SilentlyContinue
    if ($c) { return $c.Path }

    $p = "C:\ProgramData\mingw64\mingw64\bin\gcc.exe"
    if (Test-Path $p) { return $p }

    return "gcc"
}

function find-clang {

    $c = Get-Command clang -ErrorAction SilentlyContinue
    if ($c) { return $c.Path }

    $p = "C:\Program Files\LLVM\bin\clang.exe"
    if (Test-Path $p) { return $p }

    return "clang"
}

function find-clangpp {

    $c = Get-Command clang++ -ErrorAction SilentlyContinue
    if ($c) { return $c.Path }

    $p = "C:\Program Files\LLVM\bin\clang++.exe"
    if (Test-Path $p) { return $p }

    return "clang++"
}

function find-clang-format {

    $c = Get-Command clang-format -ErrorAction SilentlyContinue
    if ($c) { return $c.Path }

    $p = "C:\Program Files\LLVM\bin\clang-format.exe"
    if (Test-Path $p) { return $p }

    return "clang-format"
}

function find-cmake {

    $c = Get-Command cmake -ErrorAction SilentlyContinue
    if ($c) { return $c.Path }

    return "cmake"
}

function find-ninja {

    $c = Get-Command ninja -ErrorAction SilentlyContinue
    if ($c) { return $c.Path }

    # ninja bundled with Visual Studio
    $vs = Get-ChildItem "C:\Program Files\Microsoft Visual Studio\*\*\Common7\IDE\CommonExtensions\Microsoft\CMake\Ninja\ninja.exe" -ErrorAction SilentlyContinue |
        Sort-Object FullName -Descending | Select-Object -First 1
    if ($vs) { return $vs.FullName }

    return $null
}

$gcc = find-gcc
$clang = find-clang
$clangpp = find-clangpp
$clang_format = find-clang-format
$cmake = find-cmake

# -----------------------------
# dirs
# -----------------------------

$src_dir = "src"
$target_dir = "target"

$source = "$src_dir\main.cpp"
$target = "$target_dir\$project_name.exe"

# -----------------------------
# flags
# -----------------------------

# -municode + windows subsystem: source uses wWinMain (GUI entry), not main
# (-mwindows is MinGW-only; for the MSVC target use -Wl,/subsystem:windows)
# user32/gdi32: Win32 GUI APIs — CMake links these by default, raw clang++ does not
$ldflags_common = @("-std=c++2c", "-Wall", "-Wextra", "-municode", "-Wl,/subsystem:windows", "-luser32", "-lgdi32")
$ldflags_opt = @("-std=c++2c", "-O2", "-Wall", "-Wextra", "-g", "-municode", "-Wl,/subsystem:windows", "-luser32", "-lgdi32")

# -----------------------------
# helpers
# -----------------------------

function ensure-target {

    Remove-Item $target_dir -Recurse -Force -ErrorAction Ignore
    New-Item $target_dir -ItemType Directory | Out-Null
}

# -----------------------------
# compile clangpp
# -----------------------------

# NOTE: cannot be named "r" — built-in alias r (Invoke-History)
# outranks functions in PowerShell command resolution

function run {

    ensure-target

    & ./build.ps1 fmt

    & $clangpp $ldflags_common -o $target $source

    & $target
}

# -----------------------------
# compile clangpp optimized
# -----------------------------

function ro {

    ensure-target

    & $clangpp $ldflags_opt -o $target $source

    & $target
}

# -----------------------------
# cmake build
# -----------------------------

function cr {

    Remove-Item build -Recurse -Force -ErrorAction Ignore
    New-Item build -ItemType Directory | Out-Null

    & ./build.ps1 fmt

    $ninja = find-ninja
    if ($ninja) {
        $env:Path = "$(Split-Path $ninja);$env:Path"
        & $cmake -S . -B build -G "Ninja" `
            -D CMAKE_CXX_COMPILER="$clangpp"
    }
    else {
        # no ninja — let cmake pick the default generator
        & $cmake -S . -B build `
            -D CMAKE_CXX_COMPILER="$clangpp"
    }

    & $cmake --build build

    # cmake outputs into build\ — $target points at target\ used by r/ro/san
    $exe = Get-ChildItem build -Filter *.exe | Select-Object -First 1
    if ($exe) { & $exe.FullName }

}

# -----------------------------
# sanitize
# -----------------------------

function san($type) {

    ensure-target

    & $clangpp "-fsanitize=$type" "-g" "-municode" "-Wl,/subsystem:windows" "-luser32" "-lgdi32" $source -o $target

    & $target
}

# -----------------------------
# asm
# -----------------------------

function asm {

    ensure-target

    & $clangpp "-S" $source

}

# -----------------------------
# llvm
# -----------------------------

function ll {

    ensure-target

    & $clangpp "-S" "-emit-llvm" $source

}

# -----------------------------
# clang-format sources
# -----------------------------

function fmt {

    $exts = @(".c", ".h", ".cpp", ".hpp", ".cc", ".cxx", ".cppm", ".ixx")

    # whole tree, but skip output dirs
    # (-style=file falls back to LLVM style when no .clang-format is found)
    $files = Get-ChildItem -Recurse -File |
        Where-Object { $exts -contains $_.Extension -and $_.FullName -notmatch '\\(build|target|\.git)\\' }

    if (-not $files) {
        Write-Host "no source files found"
        return
    }

    Write-Host "fmt:" ($files.FullName -join " ")
    & $clang_format -style=file -i $files.FullName
}

# -----------------------------
# clean
# -----------------------------

function clean {

    Remove-Item target -Recurse -Force -ErrorAction Ignore
    Remove-Item build -Recurse -Force -ErrorAction Ignore
}

# -----------------------------
# init C project
# -----------------------------

function init {

    New-Item src -ItemType Directory -ErrorAction Ignore | Out-Null

@'
#include <stdio.h>

int main(void) {
    printf("Hello world C\n");
    return 0;
}
'@ | Out-File src/main.c

}

# -----------------------------
# show compilers
# -----------------------------

function compilers {

    Write-Host "gcc   :" $gcc
    Write-Host "clangpp :" $clangpp
    Write-Host "cmake :" $cmake
}

# -----------------------------
# dispatcher
# -----------------------------

switch ($cmd) {

    "r" { run }
    "ro" { ro }
    "cr" { cr }
    "san" { san $arg }
    "asm" { asm }
    "ll" { ll }
    "fmt" { fmt }
    "clean" { clean }
    "init" { init }
    "compilers" { compilers }

    default {
        Write-Host ""
        Write-Host "commands:"
        Write-Host "r ro cr san asm ll fmt clean init compilers"
    }
}
