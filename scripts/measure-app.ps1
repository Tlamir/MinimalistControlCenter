# Measures a running application and its child processes. No limits or kill flags are set.
# Run elevated for an elevated controller. Each invocation creates a fresh accounting job.
param(
    [Parameter(Mandatory)][int[]]$ProcessIds,
    [int[]]$ExternalProcessIds=@(),
    [Parameter(Mandatory)][string]$Label,
    [ValidateRange(10,120)][int]$Seconds=30,
    [Parameter(Mandatory)][string]$Output,
    [long]$WindowHandle=0
)
$ErrorActionPreference='Stop'
if (-not ('MccMeasure' -as [type])) {
    Add-Type @'
using System;
using System.Diagnostics;
using System.Runtime.InteropServices;
public sealed class MccMeasure : IDisposable {
    [DllImport("kernel32.dll",SetLastError=true)] static extern IntPtr CreateJobObjectW(IntPtr sa,IntPtr name);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool AssignProcessToJobObject(IntPtr job,IntPtr process);
    [DllImport("kernel32.dll",SetLastError=true)] static extern bool QueryInformationJobObject(IntPtr job,int kind,IntPtr data,uint size,IntPtr length);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
    [DllImport("user32.dll",SetLastError=true)] public static extern IntPtr SendMessageTimeoutW(IntPtr h,uint msg,UIntPtr w,IntPtr l,uint flags,uint timeout,out UIntPtr result);
    IntPtr job;
    public MccMeasure(int[] ids) {
        job=CreateJobObjectW(IntPtr.Zero,IntPtr.Zero);
        if(job==IntPtr.Zero) throw new System.ComponentModel.Win32Exception();
        try { foreach(int id in ids) using(var p=Process.GetProcessById(id)) {
            if(!AssignProcessToJobObject(job,p.Handle)) throw new System.ComponentModel.Win32Exception();
        }} catch {Dispose();throw;}
    }
    byte[] Query(int kind,int size) {
        IntPtr p=Marshal.AllocHGlobal(size);
        try { if(!QueryInformationJobObject(job,kind,p,(uint)size,IntPtr.Zero)) throw new System.ComponentModel.Win32Exception();
            var b=new byte[size]; Marshal.Copy(p,b,0,size);return b;
        } finally {Marshal.FreeHGlobal(p);}
    }
    public long CpuTicks() {var b=Query(1,48);return BitConverter.ToInt64(b,0)+BitConverter.ToInt64(b,8);}
    public uint TotalProcesses() {return BitConverter.ToUInt32(Query(1,48),36);}
    public ulong PeakCommit() {return BitConverter.ToUInt64(Query(9,144),136);}
    public long[] Memory() {
        var b=Query(3,16384); uint count=BitConverter.ToUInt32(b,4);
        if(count>2047) throw new Exception("Process list exceeds bound");
        long working=0,commit=0;
        for(int i=0;i<count;i++) {
            int id=(int)BitConverter.ToUInt64(b,8+i*8);
            try {using(var p=Process.GetProcessById(id)) {p.Refresh();working+=p.WorkingSet64;commit+=p.PrivateMemorySize64;}}
            catch(ArgumentException) {} catch(InvalidOperationException) {}
        }
        return new long[]{working,commit};
    }
    public static double Ping(long window) {
        UIntPtr result;var time=Stopwatch.StartNew();
        if(SendMessageTimeoutW(new IntPtr(window),0,UIntPtr.Zero,IntPtr.Zero,2,1000,out result)==IntPtr.Zero) return -1;
        return time.Elapsed.TotalMilliseconds;
    }
    public void Dispose(){if(job!=IntPtr.Zero){CloseHandle(job);job=IntPtr.Zero;}}
}
'@
}
function Percentile($values,[double]$fraction) {
    $sorted=@($values | Sort-Object)
    if (-not $sorted.Count) { return $null }
    return $sorted[[Math]::Min($sorted.Count-1,[Math]::Ceiling($sorted.Count*$fraction)-1)]
}
$job=[MccMeasure]::new($ProcessIds)
try {
    $working=@(); $commit=@(); $ping=@(); $pingFailures=0
    $external=@($ExternalProcessIds | ForEach-Object {Get-Process -Id $_})
    $externalBefore=0
    foreach($p in $external) {$externalBefore+=$p.TotalProcessorTime.Ticks}
    $before=$job.CpuTicks(); $processesBefore=$job.TotalProcesses()
    $timer=[Diagnostics.Stopwatch]::StartNew()
    while ($timer.Elapsed.TotalSeconds -lt $Seconds) {
        $mem=$job.Memory()
        foreach($p in $external) {$p.Refresh(); $mem[0]+=$p.WorkingSet64; $mem[1]+=$p.PrivateMemorySize64}
        $working+=$mem[0]; $commit+=$mem[1]
        if ($WindowHandle) {
            $duration=[MccMeasure]::Ping($WindowHandle)
            if ($duration -lt 0) { $pingFailures++ } else { $ping+=$duration }
        }
        Start-Sleep -Milliseconds 1000
    }
    $elapsed=$timer.Elapsed.TotalSeconds
    $externalAfter=0
    foreach($p in $external) {$externalAfter+=$p.TotalProcessorTime.Ticks}
    $cpuMs=($job.CpuTicks()-$before+$externalAfter-$externalBefore)/10000.0
    [ordered]@{
        label=$Label; seconds=$elapsed; samples=$working.Count
        cpu_ms_including_children=$cpuMs
        cpu_percent_one_logical_core=100*$cpuMs/($elapsed*1000)
        cpu_percent_whole_machine=100*$cpuMs/($elapsed*1000*[Environment]::ProcessorCount)
        logical_processors=[Environment]::ProcessorCount
        external_process_count=$external.Count
        median_working_set_bytes=(Percentile $working 0.5)
        max_sampled_working_set_bytes=($working|Measure-Object -Maximum).Maximum
        median_private_commit_bytes=(Percentile $commit 0.5)
        max_sampled_private_commit_bytes=($commit|Measure-Object -Maximum).Maximum
        job_peak_commit_bytes=$job.PeakCommit()
        processes_created_during_interval=$job.TotalProcesses()-$processesBefore
        window_ping_p50_ms=(Percentile $ping 0.5); window_ping_p95_ms=(Percentile $ping 0.95)
        window_ping_failures=$pingFailures
        scope='Job-assigned app processes and inherited children, including exited helpers, plus specified existing external services via process counter deltas. Children spawned by external services after attachment are not tracked. Shared WMI/driver/service hosts and benchmark process excluded. Memory sampled at 1 Hz; peak job commit excludes external services and is not interval-local. WM_NULL ping is idle message handling, not interaction latency.'
    } | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $Output -Encoding utf8
} finally {$job.Dispose()}
