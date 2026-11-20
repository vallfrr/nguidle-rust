# capbuild.ps1 <GBMax> <command> [args...] : runs the command in a memory-capped Windows Job object
# (Windows equivalent of capbuild.sh: if rustc exceeds the cap it is killed instead of filling the disk through the page file).
param([int]$GB, [Parameter(ValueFromRemainingArguments=$true)]$Cmd)
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public static class JobCap {
  [DllImport("kernel32.dll")] static extern IntPtr CreateJobObject(IntPtr a, string n);
  [DllImport("kernel32.dll")] static extern bool SetInformationJobObject(IntPtr j, int c, IntPtr p, int l);
  [DllImport("kernel32.dll")] static extern bool AssignProcessToJobObject(IntPtr j, IntPtr p);
  [StructLayout(LayoutKind.Sequential)] struct BASIC { public long a,b; public uint flags; public UIntPtr min,max; public uint act; public UIntPtr aff; public uint pri,sch; }
  [StructLayout(LayoutKind.Sequential)] struct IO { public ulong a,b,c,d,e,f; }
  [StructLayout(LayoutKind.Sequential)] struct EXT { public BASIC b; public IO io; public UIntPtr proc, job, peakp, peakj; }
  public static void Cap(System.Diagnostics.Process p, long bytes) {
    IntPtr j = CreateJobObject(IntPtr.Zero, null);
    EXT e = new EXT(); e.b.flags = 0x200; e.job = (UIntPtr)(ulong)bytes; // JOB_OBJECT_LIMIT_JOB_MEMORY
    int n = Marshal.SizeOf(typeof(EXT)); IntPtr q = Marshal.AllocHGlobal(n); Marshal.StructureToPtr(e, q, false);
    SetInformationJobObject(j, 9, q, n); AssignProcessToJobObject(j, p.Handle);
  }
}
"@
$p = Start-Process -FilePath $Cmd[0] -ArgumentList ($Cmd | Select-Object -Skip 1) -NoNewWindow -PassThru
[JobCap]::Cap($p, [long]$GB * 1GB)
$p.WaitForExit(); exit $p.ExitCode
