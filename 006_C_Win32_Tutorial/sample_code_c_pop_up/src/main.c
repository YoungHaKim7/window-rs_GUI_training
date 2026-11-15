#include <windows.h>

int WINAPI WinMain(HINSTANCE hInstance, HINSTANCE hPrevInstance, LPSTR pCmdLine,
                   int nCmdShow) {
    (void)hInstance;
    (void)hPrevInstance;
    (void)pCmdLine;
    (void)nCmdShow;

    int response = MessageBoxA(NULL,                       // No parent window
                               "Do you want to continue?", // Message text
                               "Win32 Alert",              // Window title
                               MB_YESNO | MB_ICONQUESTION  // Buttons and icon
    );

    if (response == IDYES) {
        MessageBoxA(NULL, "You clicked Yes!", "Result", MB_OK);
    }

    return 0;
}
