`extern "system"` specifies the **calling convention** (ABI) that Rust uses when calling or being called by external system-level code.

In your example, Windows calls your `wndproc` function through the Windows API. Therefore, you must use the calling convention expected by Windows.

Let's break it down.

## **1. What does&#x20;**`extern "system"`**&#x20;mean?**

```rs
extern "system" fn wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // ...
}
```

There are two important parts:

- `extern`: Declares a function that uses an external ABI.
- `"system"`: Specifies the system calling convention.

On Windows:

| Architecture    | `"system"` calling convention    |
| --------------- | -------------------------------- |
| x86 (32-bit)    | `stdcall`                        |
| x86-64 (64-bit) | Windows x64 calling convention   |
| ARM64           | Windows ARM64 calling convention |

On non-Windows platforms, `"system"` generally maps to the platform's native C ABI.

## **2. Why does&#x20;**`wndproc`**&#x20;need it?**

Look at this field in `WNDCLASSA`:

```rs
lpfnWndProc: Some(wndproc),
```

Its type is essentially:

```rs
pub lpfnWndProc: Option<
    unsafe extern "system" fn(
        HWND,
        u32,
        WPARAM,
        LPARAM,
    ) -> LRESULT
>,
```

Windows expects a callback function with this exact ABI.

When you register your window class:

```rs
RegisterClassA(&wc);
```

Windows stores your callback function pointer.

Later, when an event occurs, Windows invokes it:

```
Windows operating system
        |
        | Calls window procedure
        v
+---------------------------+
| wndproc                   |
|                           |
| HWND                      |
| message                   |
| WPARAM                    |
| LPARAM                    |
+---------------------------+
        |
        v
    Rust code
```

For example, when Windows sends `WM_PAINT`, it calls your `wndproc`.

If you use the wrong calling convention, the caller and callee may disagree about how arguments are passed, how the stack is managed, or how the return value is handled.

This can cause undefined behavior.

## **3. What happens if you use&#x20;**`extern "C"`**?**

Consider:

```rs
extern "C" fn wndproc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    0
}
```

On 64-bit Windows, `"C"` and `"system"` generally use the same calling convention.

However, on 32-bit Windows:

- `"C"` uses the C calling convention.
- `"system"` uses the Windows system calling convention (`stdcall`).

They are not interchangeable there.

You should use `"system"` for Windows API callbacks.

## **4. Difference between&#x20;**`fn`**,&#x20;**`extern "C" fn`**, and&#x20;**`extern "system" fn`

| Declaration                       | Meaning                     |
| --------------------------------- | --------------------------- |
| `fn foo()`                        | Rust ABI                    |
| `extern "C" fn foo()`             | C ABI                       |
| `extern "system" fn foo()`        | Native system ABI           |
| `unsafe extern "system" fn foo()` | System ABI, unsafe function |

One important distinction:

`extern "system"` does not automatically mean the function is unsafe.

For example:

```rs
extern "system" fn callback() {
    println!("Hello");
}
```

And:

```rs
unsafe extern "system" fn callback() {
    println!("Hello");
}
```

Both specify the same calling convention, but differ in whether callers must use an unsafe context.

## **5. How to inspect the generated assembly**

Since you're learning Windows API and low-level programming, you can inspect the generated assembly.

Try:

```rs
extern "system" fn wndproc() -> i32 {
    0
}

extern "C" fn c_callback() -> i32 {
    0
}

fn main() {}
```

Compile for 64-bit Windows:

```
rustc --emit=asm --target x86_64-pc-windows-msvc main.rs
```

On x86-64 Windows, both functions use the same underlying calling convention.

- **The key takeaway:** `extern "system"` tells Rust how Windows is allowed to call your callback function. It is essential when implementing Windows API callbacks such as `WNDPROC`, thread entry points, and other Windows-defined function pointers.
