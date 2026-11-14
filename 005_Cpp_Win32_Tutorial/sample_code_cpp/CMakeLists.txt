cmake_minimum_required(VERSION 3.20)

project(Cpp_lang CXX)

set(CMAKE_CXX_STANDARD 26)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# name the exe after the containing folder (same as build.ps1's $project_name)
get_filename_component(folder_name ${CMAKE_CURRENT_SOURCE_DIR} NAME)

add_executable(Cpp_lang WIN32 src/main.cpp)
set_target_properties(Cpp_lang PROPERTIES OUTPUT_NAME ${folder_name})
