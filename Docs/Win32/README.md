# 그림 출처

### 국내 유료 win32강의 인프런(Windows System 프로그래밍)
- Windows System 프로그래밍 | Windows용 게임과 어플리케이션을 개발하기 위한 필수 Windows System프로그래밍 기술을 알려드립니다.
  - https://www.inflearn.com/course/windows-system-programming?cid=336247

<img width="2514" height="1417" alt="Image" src="https://github.com/user-attachments/assets/0c227956-c139-4bc6-a20c-8424c509b990" />


# 여기에 정리함
- https://younghakim7.github.io/blog/posts/study_windowsos_kernel_001/

### category
- https://younghakim7.github.io/blog/archive/category/rust_Kernel_WindowsOS/

Let's visualize the Windows OS architecture entirely in the terminal, using ASCII diagrams. You can save these diagrams as `.txt` files and view them in Helix, Vim, Neovim, or directly in your terminal.

The goal is to understand how Windows connects user applications, system calls, the kernel, kernel managers, device drivers, and hardware.

# 1. Overall Windows architecture

```
+----------------------------------------------------------+
|                     USER MODE                            |
|                                                          |
|  +----------------+  +----------------+  +-------------+ |
|  | Application A  |  | Application B  |  | Application C|
|  +-------+--------+  +-------+--------+  +------+------ + |
|          |                   |                  |        |
|  +-------v-------------------v------------------v------+ |
|  |                  Win32 API                           | |
|  |  KernelBase.dll | User32.dll | Gdi32.dll | Advapi32  | |
|  +--------------------------+---------------------------+ |
|                             |                            |
|  +--------------------------v---------------------------+ |
|  |                    ntdll.dll                          | |
|  |          Native API / System Call Stubs               | |
|  +--------------------------+---------------------------+ |
+=============================|============================+
                              |
                       SYSTEM CALL
                         SYSCALL
                              |
+=============================|============================+
|                     KERNEL MODE                          |
|  +--------------------------v---------------------------+ |
|  |                    ntoskrnl.exe                       | |
|  |                                                      | |
|  |  +-----------------------------------------------+   | |
|  |  |              Windows Executive                |   | |
|  |  |                                               |   | |
|  |  |  +----------------+  +---------------------+  |   | |
|  |  |  | Object Manager |  | Process Manager     |  |   | |
|  |  |  +----------------+  +---------------------+  |   | |
|  |  |                                               |   | |
|  |  |  +----------------+  +---------------------+  |   | |
|  |  |  | Memory Manager |  | I/O Manager         |  |   | |
|  |  |  +----------------+  +---------------------+  |   | |
|  |  |                                               |   | |
|  |  |  +----------------+  +---------------------+  |   | |
|  |  |  | Security       |  | Cache Manager       |  |   | |
|  |  |  +----------------+  +---------------------+  |   | |
|  |  +-----------------------------------------------+   | |
|  |                                                      | |
|  |  +-----------------------------------------------+   | |
|  |  |              Windows Kernel                   |   | |
|  |  |                                               |   | |
|  |  |  Scheduler | Interrupts | Synchronization     |   | |
|  |  |  Dispatcher | Exceptions | Timers             |   | |
|  |  +-----------------------------------------------+   | |
|  +------------------------------------------------------+ |
|                                                          |
|  +------------------------------------------------------+ |
|  |                  HAL (hal.dll)                       | |
|  +------------------------------------------------------+ |
|                                                          |
|  +------------------------------------------------------+ |
|  |                Kernel-mode Drivers                   | |
|  |                                                      | |
|  |  File System | Storage | Network | USB | GPU         | |
|  +----------------------------+-------------------------+ |
+===============================|==========================+
                                |
+===============================v==========================+
|                         HARDWARE                         |
|                                                          |
|  +----------+ +----------+ +----------+ +-------------+ |
|  | CPU      | | RAM      | | SSD      | | GPU         | |
|  +----------+ +----------+ +----------+ +-------------+ |
|                                                          |
|  +----------+ +----------+ +--------------------------+ |
|  | Keyboard | | Mouse    | | Network Controller       | |
|  +----------+ +----------+ +--------------------------+ |
+----------------------------------------------------------+
```

Important: This is a conceptual diagram, not a literal executable-module hierarchy. The Executive and Kernel are closely integrated within `ntoskrnl.exe`; HAL and drivers are separate kernel-mode components.

# 2. User mode vs. kernel mode

The most important boundary in Windows is the transition between user mode and kernel mode.

```
              USER MODE
     +--------------------------+
     |                          |
     |   Application            |
     |       |                  |
     |       v                  |
     |   Win32 API              |
     |       |                  |
     |       v                  |
     |   ntdll.dll              |
     |       |                  |
     +-------|------------------+
             |
             | SYSCALL
             |
=============|====================== PRIVILEGE BOUNDARY
             |
             v
              KERNEL MODE
     +--------------------------+
     |                          |
     |   System Service         |
     |       |                  |
     |       v                  |
     |   Executive              |
     |       |                  |
     |       v                  |
     |   Kernel                 |
     |       |                  |
     |       v                  |
     |   Hardware / Drivers     |
     |                          |
     +--------------------------+
```

On x86-64 Windows:

| Mode        | CPU privilege level |
| ----------- | ------------------- |
| User mode   | CPL 3               |
| Kernel mode | CPL 0               |

The `SYSCALL` instruction is one mechanism used to transition into kernel mode.

# 3. Windows Executive architecture

The Executive provides higher-level operating system services.

```
                 WINDOWS EXECUTIVE
+--------------------------------------------------+
|                                                  |
|  +--------------------+  +---------------------+ |
|  | Object Manager     |  | Process Manager     | |
|  |                    |  |                     | |
|  | Objects            |  | Processes           | |
|  | Handles            |  | Threads             | |
|  | Namespaces         |  | Jobs                | |
|  +--------------------+  +---------------------+ |
|                                                  |
|  +--------------------+  +---------------------+ |
|  | Memory Manager     |  | I/O Manager         | |
|  |                    |  |                     | |
|  | Virtual Memory     |  | IRPs                | |
|  | Physical Memory    |  | Device Objects      | |
|  | Page Faults        |  | Driver Dispatch     | |
|  +--------------------+  +---------------------+ |
|                                                  |
|  +--------------------+  +---------------------+ |
|  | Security Reference |  | Cache Manager       | |
|  | Monitor            |  |                     | |
|  |                    |  | File Cache          | |
|  | Access Control     |  | Memory Mapping      | |
|  +--------------------+  +---------------------+ |
|                                                  |
+--------------------------------------------------+
```

Study each manager separately.

| Manager                    | Main responsibility               |
| -------------------------- | --------------------------------- |
| Object Manager             | Objects, handles, namespaces      |
| Process Manager            | Processes and threads             |
| Memory Manager             | Virtual memory and physical pages |
| I/O Manager                | Communication with device drivers |
| Security Reference Monitor | Access checking                   |
| Cache Manager              | File-system caching               |

# 4. Windows process creation flow

Let's follow a real operation.

An application calls:

```
CreateProcessW(
    L"C:\\Windows\\System32\\notepad.exe",
    NULL,
    NULL,
    NULL,
    FALSE,
    0,
    NULL,
    NULL,
    &startup_info,
    &process_info
);
```

Conceptual execution flow:

```
+-----------------------------+
| Application                 |
+--------------+--------------+
               |
               v
+-----------------------------+
| CreateProcessW()            |
| KernelBase.dll              |
+--------------+--------------+
               |
               v
+-----------------------------+
| Native API / ntdll.dll      |
+--------------+--------------+
               |
               v
+-----------------------------+
| System Call                 |
+--------------+--------------+
               |
               v
+-----------------------------+
| Windows Kernel              |
+--------------+--------------+
               |
               v
+-----------------------------+
| Process Manager             |
|                             |
| - Create process            |
| - Create initial thread     |
| - Initialize process state  |
+--------------+--------------+
               |
               v
+-----------------------------+
| Memory Manager              |
|                             |
| - Address space             |
| - Virtual memory            |
| - Page tables               |
+--------------+--------------+
               |
               v
+-----------------------------+
| Object Manager              |
|                             |
| - Process object            |
| - Thread object             |
| - Handles                   |
+--------------+--------------+
               |
               v
+-----------------------------+
| New process execution       |
+-----------------------------+
```

This is a simplified conceptual flow: process creation involves several cooperating subsystems, and their exact execution order is more complicated.

# 5. Windows memory management

Since you have been studying x86-64 page tables and MMU implementation, this is especially important.

```
          VIRTUAL MEMORY
+--------------------------------+
| Process A Address Space         |
|                                |
|  0x0000000000400000             |
|  +--------------------------+  |
|  | Executable               |  |
|  +--------------------------+  |
|                                |
|  0x0000000010000000             |
|  +--------------------------+  |
|  | Heap                     |  |
|  +--------------------------+  |
|                                |
|  0x00007FFFFFFFF000             |
|  +--------------------------+  |
|  | Stack                    |  |
|  +--------------------------+  |
+----------------+---------------+
                 |
                 v
+--------------------------------+
| Windows Memory Manager         |
|                                |
| Virtual Address Management     |
| Page Fault Handling            |
| Working Set Management         |
| Physical Page Management       |
+----------------+---------------+
                 |
                 v
+--------------------------------+
| Hardware Page Tables           |
|                                |
| PML4                           |
|  |                             |
|  +--> PDPT                     |
|        |                       |
|        +--> PD                  |
|               |                |
|               +--> PT           |
+----------------+---------------+
                 |
                 v
+--------------------------------+
| Physical RAM                   |
+--------------------------------+
```

Remember: Windows Memory Manager manages memory policies and mappings, while the CPU's MMU performs hardware address translation.

# 6. Windows interrupt handling

```
             HARDWARE DEVICE
                    |
                    v
             Hardware Interrupt
                    |
                    v
          +--------------------+
          | Interrupt Dispatch |
          +---------+----------+
                    |
                    v
          +--------------------+
          | ISR                |
          | Interrupt Service  |
          | Routine            |
          +---------+----------+
                    |
                    v
          +--------------------+
          | DPC                |
          | Deferred Procedure |
          | Call               |
          +---------+----------+
                    |
                    v
          +--------------------+
          | Kernel Scheduler   |
          +---------+----------+
                    |
                    v
          +--------------------+
          | Thread Execution   |
          +--------------------+
```

Important concepts:

- IDT: Interrupt Descriptor Table.
- ISR: Interrupt Service Routine.
- IRQL: Interrupt Request Level.
- DPC: Deferred Procedure Call.
- APC: Asynchronous Procedure Call.

Note that this is a simplified path. Not every interrupt requires a DPC or causes a context switch.

# 7. Windows I/O architecture

For example, when an application reads a file:

```
+--------------------------+
| Application              |
|                          |
| ReadFile()               |
+------------+-------------+
             |
             v
+--------------------------+
| Windows API              |
+------------+-------------+
             |
             v
+--------------------------+
| I/O Manager               |
|                          |
| Creates IRP              |
+------------+-------------+
             |
             v
+--------------------------+
| File System Driver       |
|                          |
| NTFS                     |
+------------+-------------+
             |
             v
+--------------------------+
| Storage Driver           |
+------------+-------------+
             |
             v
+--------------------------+
| Disk Controller          |
+------------+-------------+
             |
             v
+--------------------------+
| SSD / HDD                |
+--------------------------+
```

The central concept here is `IRP` (I/O Request Packet).

# 8. Generate and view the architecture from your terminal

Here is a practical way to keep these diagrams in your Windows kernel study repository.

Suggested directory structure:

```
windows-kernel-study/
|
+-- README.md
|
+-- architecture/
|   |
|   +-- 01-overall.txt
|   +-- 02-user-kernel.txt
|   +-- 03-executive.txt
|   +-- 04-process.txt
|   +-- 05-memory.txt
|   +-- 06-interrupt.txt
|   +-- 07-io.txt
|
+-- exercises/
    |
    +-- process/
    +-- memory/
    +-- interrupts/
    +-- drivers/
    +-- debugging/
```

You can use `cat`, `less`, and `hx` (Helix).

```
# Display directly
cat architecture/01-overall.txt

# Scroll through the diagram
less architecture/01-overall.txt

# Open with Helix
hx architecture/01-overall.txt
```

You can also use Mermaid diagrams in Markdown and render them with a Mermaid CLI installation:

```
npm install -g @mermaid-js/mermaid-cli
```

Example `architecture/overall.mmd`:

\#chatgpt-mermaid-\_r_nb\_{font-family:-apple-system-body,ui-sans-serif,-apple-system,system-ui,Segoe UI,Helvetica,Apple Color Emoji,Arial,sans-serif,Segoe UI Emoji,Segoe UI Symbol;font-size:16px;fill:rgb(237, 237, 237);}@keyframes edge-animation-frame{from{stroke-dashoffset:0;}}@keyframes dash{to{stroke-dashoffset:0;}}#chatgpt-mermaid-\_r_nb\_ .edge-animation-slow{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 50s linear infinite;stroke-linecap:round;}#chatgpt-mermaid-\_r_nb\_ .edge-animation-fast{stroke-dasharray:9,5!important;stroke-dashoffset:900;animation:dash 20s linear infinite;stroke-linecap:round;}#chatgpt-mermaid-\_r_nb\_ .error-icon{fill:rgb(48, 48, 48);}#chatgpt-mermaid-\_r_nb\_ .error-text{fill:rgb(237, 237, 237);stroke:rgb(237, 237, 237);}#chatgpt-mermaid-\_r_nb\_ .edge-thickness-normal{stroke-width:1px;}#chatgpt-mermaid-\_r_nb\_ .edge-thickness-thick{stroke-width:3.5px;}#chatgpt-mermaid-\_r_nb\_ .edge-pattern-solid{stroke-dasharray:0;}#chatgpt-mermaid-\_r_nb\_ .edge-thickness-invisible{stroke-width:0;fill:none;}#chatgpt-mermaid-\_r_nb\_ .edge-pattern-dashed{stroke-dasharray:3;}#chatgpt-mermaid-\_r_nb\_ .edge-pattern-dotted{stroke-dasharray:2;}#chatgpt-mermaid-\_r_nb\_ .marker{fill:rgb(175, 175, 175);stroke:rgb(175, 175, 175);}#chatgpt-mermaid-\_r_nb\_ .marker.cross{stroke:rgb(175, 175, 175);}#chatgpt-mermaid-\_r_nb\_ svg{font-family:-apple-system-body,ui-sans-serif,-apple-system,system-ui,Segoe UI,Helvetica,Apple Color Emoji,Arial,sans-serif,Segoe UI Emoji,Segoe UI Symbol;font-size:16px;}#chatgpt-mermaid-\_r_nb\_ p{margin:0;}#chatgpt-mermaid-\_r_nb\_ .label{font-family:-apple-system-body,ui-sans-serif,-apple-system,system-ui,Segoe UI,Helvetica,Apple Color Emoji,Arial,sans-serif,Segoe UI Emoji,Segoe UI Symbol;color:rgb(237, 237, 237);}#chatgpt-mermaid-\_r_nb\_ .cluster-label text{fill:rgb(237, 237, 237);}#chatgpt-mermaid-\_r_nb\_ .cluster-label span{color:rgb(237, 237, 237);}#chatgpt-mermaid-\_r_nb\_ .cluster-label span p{background-color:transparent;}#chatgpt-mermaid-\_r_nb\_ .label text,#chatgpt-mermaid-\_r_nb\_ span{fill:rgb(237, 237, 237);color:rgb(237, 237, 237);}#chatgpt-mermaid-\_r_nb\_ .node rect,#chatgpt-mermaid-\_r_nb\_ .node circle,#chatgpt-mermaid-\_r_nb\_ .node ellipse,#chatgpt-mermaid-\_r_nb\_ .node polygon,#chatgpt-mermaid-\_r_nb\_ .node path{fill:rgb(9, 23, 44);stroke:rgb(31, 78, 148);stroke-width:1px;}#chatgpt-mermaid-\_r_nb\_ .rough-node .label text,#chatgpt-mermaid-\_r_nb\_ .node .label text,#chatgpt-mermaid-\_r_nb\_ .image-shape .label,#chatgpt-mermaid-\_r_nb\_ .icon-shape .label{text-anchor:middle;}#chatgpt-mermaid-\_r_nb\_ .node .katex path{fill:#000;stroke:#000;stroke-width:1px;}#chatgpt-mermaid-\_r_nb\_ .rough-node .label,#chatgpt-mermaid-\_r_nb\_ .node .label,#chatgpt-mermaid-\_r_nb\_ .image-shape .label,#chatgpt-mermaid-\_r_nb\_ .icon-shape .label{text-align:center;}#chatgpt-mermaid-\_r_nb\_ .node.clickable{cursor:pointer;}#chatgpt-mermaid-\_r_nb\_ .root .anchor path{fill:rgb(175, 175, 175)!important;stroke-width:0;stroke:rgb(175, 175, 175);}#chatgpt-mermaid-\_r_nb\_ .arrowheadPath{fill:rgb(175, 175, 175);}#chatgpt-mermaid-\_r_nb\_ .edgePath .path{stroke:rgb(175, 175, 175);stroke-width:1px;}#chatgpt-mermaid-\_r_nb\_ .flowchart-link{stroke:rgb(175, 175, 175);fill:none;}#chatgpt-mermaid-\_r_nb\_ .edgeLabel{background-color:rgb(0, 0, 0);text-align:center;}#chatgpt-mermaid-\_r_nb\_ .edgeLabel p{background-color:rgb(0, 0, 0);}#chatgpt-mermaid-\_r_nb\_ .edgeLabel rect{opacity:0.5;background-color:rgb(0, 0, 0);fill:rgb(0, 0, 0);}#chatgpt-mermaid-\_r_nb\_ .labelBkg{background-color:rgba(0, 0, 0, 0.5);}#chatgpt-mermaid-\_r_nb\_ .cluster rect{fill:rgb(48, 48, 48);stroke:rgba(255, 255, 255, 0.15);stroke-width:1px;}#chatgpt-mermaid-\_r_nb\_ .cluster text{fill:rgb(237, 237, 237);}#chatgpt-mermaid-\_r_nb\_ .cluster span{color:rgb(237, 237, 237);}#chatgpt-mermaid-\_r_nb\_ div.mermaidTooltip{position:absolute;text-align:center;max-width:200px;padding:2px;font-family:-apple-system-body,ui-sans-serif,-apple-system,system-ui,Segoe UI,Helvetica,Apple Color Emoji,Arial,sans-serif,Segoe UI Emoji,Segoe UI Symbol;font-size:12px;background:rgb(48, 48, 48);border:1px solid rgba(255, 255, 255, 0.15);border-radius:2px;pointer-events:none;z-index:100;}#chatgpt-mermaid-\_r_nb\_ .flowchartTitleText{text-anchor:middle;font-size:18px;fill:rgb(237, 237, 237);}#chatgpt-mermaid-\_r_nb\_ rect.text{fill:none;stroke-width:0;}#chatgpt-mermaid-\_r_nb\_ .icon-shape,#chatgpt-mermaid-\_r_nb\_ .image-shape{background-color:rgb(0, 0, 0);text-align:center;}#chatgpt-mermaid-\_r_nb\_ .icon-shape p,#chatgpt-mermaid-\_r_nb\_ .image-shape p{background-color:rgb(0, 0, 0);padding:2px;}#chatgpt-mermaid-\_r_nb\_ .icon-shape .label rect,#chatgpt-mermaid-\_r_nb\_ .image-shape .label rect{opacity:0.5;background-color:rgb(0, 0, 0);fill:rgb(0, 0, 0);}#chatgpt-mermaid-\_r_nb\_ .label-icon{display:inline-block;height:1em;overflow:visible;vertical-align:-0.125em;}#chatgpt-mermaid-\_r_nb\_ .node .label-icon path{fill:currentColor;stroke:revert;stroke-width:revert;}#chatgpt-mermaid-\_r_nb\_ .node .neo-node{stroke:rgb(31, 78, 148);}#chatgpt-mermaid-\_r_nb\_ [data-look="neo"].node rect,#chatgpt-mermaid-\_r_nb\_ [data-look="neo"].cluster rect,#chatgpt-mermaid-\_r_nb\_ [data-look="neo"].node polygon{stroke:url(#chatgpt-mermaid-\_r_nb\_-gradient);filter:drop-shadow( 1px 2px 2px rgba(185,185,185,1));}#chatgpt-mermaid-\_r_nb\_ [data-look="neo"].swimlane.cluster rect{filter:none;}#chatgpt-mermaid-\_r_nb\_ [data-look="neo"].node path{stroke:url(#chatgpt-mermaid-\_r_nb\_-gradient);stroke-width:1px;}#chatgpt-mermaid-\_r_nb\_ [data-look="neo"].node .outer-path{filter:drop-shadow( 1px 2px 2px rgba(185,185,185,1));}#chatgpt-mermaid-\_r_nb\_ [data-look="neo"].node .neo-line path{stroke:rgb(31, 78, 148);filter:none;}#chatgpt-mermaid-\_r_nb\_ [data-look="neo"].node circle{stroke:url(#chatgpt-mermaid-\_r_nb\_-gradient);filter:drop-shadow( 1px 2px 2px rgba(185,185,185,1));}#chatgpt-mermaid-\_r_nb\_ [data-look="neo"].node circle .state-start{fill:#000000;}#chatgpt-mermaid-\_r_nb\_ [data-look="neo"].icon-shape .icon{fill:url(#chatgpt-mermaid-\_r_nb\_-gradient);filter:drop-shadow( 1px 2px 2px rgba(185,185,185,1));}#chatgpt-mermaid-\_r_nb\_ [data-look="neo"].icon-shape .icon-neo path{stroke:url(#chatgpt-mermaid-\_r_nb\_-gradient);filter:drop-shadow( 1px 2px 2px rgba(185,185,185,1));}#chatgpt-mermaid-\_r_nb\_ .node text{font-size:14px;font-weight:600;letter-spacing:normal;fill:rgb(153, 206, 255);}#chatgpt-mermaid-\_r_nb\_ .edgeLabels text{font-size:13px;font-weight:600;letter-spacing:-0.08px;fill:rgb(153, 206, 255);}#chatgpt-mermaid-\_r_nb\_ .node tspan[font-weight="normal"],#chatgpt-mermaid-\_r_nb\_ .edgeLabels tspan[font-weight="normal"]{font-weight:600;}#chatgpt-mermaid-\_r_nb\_ .edgeLabel .label rect{opacity:1;rx:13px;ry:13px;fill:rgb(0, 14, 26);stroke:rgb(26, 62, 95);stroke-width:1px;}#chatgpt-mermaid-\_r_nb\_ .node rect,#chatgpt-mermaid-\_r_nb\_ .node circle,#chatgpt-mermaid-\_r_nb\_ .node ellipse,#chatgpt-mermaid-\_r_nb\_ .node polygon,#chatgpt-mermaid-\_r_nb\_ .node path{fill:rgb(0, 40, 77);stroke:rgba(255, 255, 255, 0.1);stroke-width:1px;}#chatgpt-mermaid-\_r_nb\_ .node rect{rx:16px;ry:16px;}#chatgpt-mermaid-\_r_nb\_ .mermaid-decision .label-container{fill:rgb(0, 14, 26);stroke:rgb(26, 62, 95);stroke-dasharray:2px,2px;}#chatgpt-mermaid-\_r_nb\_ .edgePaths .flowchart-link{stroke:rgb(175, 175, 175);stroke-width:1px;stroke-linecap:round;stroke-linejoin:round;}#chatgpt-mermaid-\_r_nb\_ .marker{fill:rgb(175, 175, 175);stroke:rgb(175, 175, 175);}#chatgpt-mermaid-\_r_nb\_ :root{--mermaid-font-family:-apple-system-body,ui-sans-serif,-apple-system,system-ui,Segoe UI,Helvetica,Apple Color Emoji,Arial,sans-serif,Segoe UI Emoji,Segoe UI Symbol;}KERNEL MODEUSER MODEWindows ExecutiveObject ManagerProcess ManagerMemory ManagerI/O ManagerWindows KernelHALKernel-mode DriversApplicationsWin32 APIntdll.dllSystem Call BoundaryHardware

Render it:

```
mmdc -i architecture/overall.mmd \
     -o architecture/overall.svg
```

# 9. What to study next

I recommend following this exact progression:

## Windows kernel learning sequence

1. Windows architecture

   Understand user mode, kernel mode, Executive, Kernel, HAL, and drivers.
2. System calls

   Follow `CreateFileW()` from Win32 to the kernel.
3. Process and thread management

   Investigate `EPROCESS`, `ETHREAD`, and `KTHREAD`.
4. Memory management

   Investigate virtual addresses, PML4, PTEs, and page faults.
5. Interrupts and scheduling

   Investigate ISR, DPC, APC, IRQL, and context switching.
6. I/O and drivers

   Investigate IRPs, device objects, and driver stacks.

My recommendation: Start with diagrams 1–3, then investigate each subsystem using WinDbg. Compare the Windows concepts against your own Rust x86-64 operating system implementation. This will help you understand not just what each Windows component does, but why an operating system needs it.
