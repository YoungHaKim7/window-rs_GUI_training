cmake_minimum_required(VERSION 3.20)

project(C23_lang C)

set(CMAKE_C_STANDARD 23)
set(CMAKE_C_STANDARD_REQUIRED ON)

# name the exe after the containing folder (same as build.ps1's $project_name)
get_filename_component(folder_name ${CMAKE_CURRENT_SOURCE_DIR} NAME)

add_executable(Cpp_lang WIN32 src/main.c)
set_target_properties(C_lang PROPERTIES OUTPUT_NAME ${folder_name})
