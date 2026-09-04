# Event-based OpenBitFun process-tree probe for Windows.
#
# This probe subscribes to Win32 process start/stop events instead of polling a
# fixed list of process names. It records every process event seen while it is
# active, then attributes each process to the OpenBitFun root using a parent table
# that survives parent-process exit.
#
# Run from a separate PowerShell window. Run as Administrator when possible so
# command lines and executable paths are available for all processes.
#
# Examples:
#   .\probe-openbitfun-process-events.ps1 -DurationSec 60 -OutputPath .\openbitfun-process-events.log
#   .\probe-openbitfun-process-events.ps1 -OpenBitFunPid 8352 -DurationSec 120

[CmdletBinding()]
param(
    [ValidateRange(1, 3600)]
    [int]$DurationSec = 60,

    [int[]]$OpenBitFunPid = @(),

    [string]$OutputPath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Get-OpenBitFunProcessIds {
    @(
        Get-CimInstance Win32_Process |
            Where-Object { $_.Name -ieq 'openbitfun-desktop.exe' } |
            ForEach-Object { [int]$_.ProcessId }
    )
}

function New-ProcessSnapshot {
    param(
        [Parameter(Mandatory)]
        [int]$ProcessId,

        [int]$ParentProcessId,

        [Parameter(Mandatory)]
        [string]$Name,

        [string]$ExecutablePath,

        [string]$CommandLine
    )

    [PSCustomObject]@{
        ProcessId = $ProcessId
        ParentProcessId = $ParentProcessId
        Name = $Name
        ExecutablePath = $ExecutablePath
        CommandLine = $CommandLine
    }
}

function Format-LogValue {
    param(
        [AllowNull()]
        [object]$Value
    )

    if ($null -eq $Value) {
        return '-'
    }

    $text = [string]$Value
    $text = $text.Replace("`r", '\r').Replace("`n", '\n').Replace("`t", '\t').Replace('"', '\"')
    '"' + $text + '"'
}

$rootProcessIds = if (@($OpenBitFunPid).Count -gt 0) {
    @($OpenBitFunPid | ForEach-Object { [int]$_ } | Select-Object -Unique)
} else {
    @(Get-OpenBitFunProcessIds)
}

if (@($rootProcessIds).Count -eq 0) {
    throw 'No openbitfun-desktop.exe process was found. Pass -OpenBitFunPid explicitly or start OpenBitFun first.'
}

$probeProcessId = $PID
$probeStartedAt = (Get-Date).ToString('o')
$isElevated = try {
    $principal = [Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
    $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
} catch {
    $false
}
$knownProcesses = [hashtable]::Synchronized(@{})
$records = [System.Collections.Concurrent.ConcurrentQueue[object]]::new()
$callbackErrors = [System.Collections.Concurrent.ConcurrentQueue[object]]::new()

# Seed the parent table so processes that already exist when the probe starts
# can still participate in lineage attribution for later descendants.
Get-CimInstance Win32_Process | ForEach-Object {
    $knownProcesses[[int]$_.ProcessId] = New-ProcessSnapshot `
        -ProcessId ([int]$_.ProcessId) `
        -ParentProcessId ([int]$_.ParentProcessId) `
        -Name ([string]$_.Name) `
        -ExecutablePath ([string]$_.ExecutablePath) `
        -CommandLine ([string]$_.CommandLine)
}

$state = [hashtable]::Synchronized(@{
    RootProcessIds = $rootProcessIds
    ProbeProcessId = $probeProcessId
    KnownProcesses = $knownProcesses
    Records = $records
    CallbackErrors = $callbackErrors
})

$startQuery = New-Object System.Management.WqlEventQuery
$startQuery.QueryString = 'SELECT * FROM Win32_ProcessStartTrace'
$startWatcher = New-Object System.Management.ManagementEventWatcher($startQuery)
$stopWatcher = $null
$startSubscription = $null
$stopSubscription = $null

try {
    $startSubscription = Register-ObjectEvent `
        -InputObject $startWatcher `
        -EventName EventArrived `
        -MessageData $state `
        -Action {
        try {
            $state = $event.MessageData
            $eventData = $eventArgs.NewEvent
            $processId = [int]$eventData.ProcessID
            if ($processId -eq [int]$state.ProbeProcessId) {
                return
            }

            $detail = Get-CimInstance Win32_Process -Filter "ProcessId = $processId" -ErrorAction SilentlyContinue
            $snapshot = if ($detail) {
                [PSCustomObject]@{
                    ProcessId = [int]$detail.ProcessId
                    ParentProcessId = [int]$detail.ParentProcessId
                    Name = [string]$detail.Name
                    ExecutablePath = [string]$detail.ExecutablePath
                    CommandLine = [string]$detail.CommandLine
                }
            } else {
                [PSCustomObject]@{
                    ProcessId = $processId
                    ParentProcessId = [int]$eventData.ParentProcessID
                    Name = [string]$eventData.ProcessName
                    ExecutablePath = $null
                    CommandLine = $null
                }
            }
            $state.KnownProcesses[$processId] = $snapshot

            $lineage = [System.Collections.Generic.List[string]]::new()
            $currentPid = [int]$snapshot.ParentProcessId
            $isOpenBitFunDescendant = $false
            $isComplete = $true
            for ($depth = 0; $depth -lt 32 -and $currentPid -gt 0; $depth++) {
                if ($state.RootProcessIds -contains $currentPid) {
                    $isOpenBitFunDescendant = $true
                    break
                }
                if (-not $state.KnownProcesses.Contains($currentPid)) {
                    $isComplete = $false
                    break
                }
                $current = $state.KnownProcesses[$currentPid]
                $lineage.Add("$($current.Name)#$($current.ProcessId)")
                $currentPid = [int]$current.ParentProcessId
            }
            if ($currentPid -eq 0) {
                $isComplete = $false
            }

            $state.Records.Enqueue([PSCustomObject]@{
                EventType = 'start'
                ObservedAt = (Get-Date).ToString('o')
                ProcessName = $snapshot.Name
                ProcessId = $snapshot.ProcessId
                ParentProcessId = $snapshot.ParentProcessId
                ExecutablePath = $snapshot.ExecutablePath
                CommandLine = $snapshot.CommandLine
                IsOpenBitFunDescendant = $isOpenBitFunDescendant
                AttributionStatus = if ($isOpenBitFunDescendant) {
                    if ($state.RootProcessIds -contains $snapshot.ParentProcessId) { 'direct' } else { 'descendant' }
                } elseif ($isComplete) { 'not_openbitfun' } else { 'unknown' }
                LineageComplete = $isComplete
                Lineage = $lineage.ToArray()
            })
        }
        catch {
            $event.MessageData.CallbackErrors.Enqueue([PSCustomObject]@{
                EventType = 'start'
                ObservedAt = (Get-Date).ToString('o')
                Message = $_.Exception.Message
            })
        }
    }

    $stopQuery = New-Object System.Management.WqlEventQuery
    $stopQuery.QueryString = 'SELECT * FROM Win32_ProcessStopTrace'
    $stopWatcher = New-Object System.Management.ManagementEventWatcher($stopQuery)
    $stopSubscription = Register-ObjectEvent `
        -InputObject $stopWatcher `
        -EventName EventArrived `
        -MessageData $state `
        -Action {
        try {
            $state = $event.MessageData
            $eventData = $eventArgs.NewEvent
            $processId = [int]$eventData.ProcessID
            if ($processId -eq [int]$state.ProbeProcessId) {
                return
            }

            $known = $state.KnownProcesses[$processId]
            $processName = if ($known) { $known.Name } else { [string]$eventData.ProcessName }
            $parentProcessId = if ($known) {
                [int]$known.ParentProcessId
            } elseif ($eventData.PSObject.Properties['ParentProcessID']) {
                [int]$eventData.ParentProcessID
            } else {
                0
            }

            $lineage = [System.Collections.Generic.List[string]]::new()
            $currentPid = $parentProcessId
            $isOpenBitFunDescendant = $false
            $isComplete = $true
            for ($depth = 0; $depth -lt 32 -and $currentPid -gt 0; $depth++) {
                if ($state.RootProcessIds -contains $currentPid) {
                    $isOpenBitFunDescendant = $true
                    break
                }
                if (-not $state.KnownProcesses.Contains($currentPid)) {
                    $isComplete = $false
                    break
                }
                $current = $state.KnownProcesses[$currentPid]
                $lineage.Add("$($current.Name)#$($current.ProcessId)")
                $currentPid = [int]$current.ParentProcessId
            }
            if ($currentPid -eq 0) {
                $isComplete = $false
            }

            $state.Records.Enqueue([PSCustomObject]@{
                EventType = 'stop'
                ObservedAt = (Get-Date).ToString('o')
                ProcessName = $processName
                ProcessId = $processId
                ParentProcessId = $parentProcessId
                ExecutablePath = if ($known) { $known.ExecutablePath } else { $null }
                CommandLine = if ($known) { $known.CommandLine } else { $null }
                IsOpenBitFunDescendant = $isOpenBitFunDescendant
                AttributionStatus = if ($isOpenBitFunDescendant) { 'descendant' } elseif ($isComplete) { 'not_openbitfun' } else { 'unknown' }
                LineageComplete = $isComplete
                Lineage = $lineage.ToArray()
            })
        }
        catch {
            $event.MessageData.CallbackErrors.Enqueue([PSCustomObject]@{
                EventType = 'stop'
                ObservedAt = (Get-Date).ToString('o')
                Message = $_.Exception.Message
            })
        }
    }
}
catch {
    $message = $_.Exception.Message
    if ($null -ne $startSubscription) {
        Unregister-Event -SubscriptionId $startSubscription.Id -ErrorAction SilentlyContinue
    }
    if ($null -ne $stopSubscription) {
        Unregister-Event -SubscriptionId $stopSubscription.Id -ErrorAction SilentlyContinue
    }
    $startWatcher.Dispose()
    if ($null -ne $stopWatcher) {
        $stopWatcher.Dispose()
    }
    if ($message -match 'Access denied') {
        throw 'Process event subscription was denied. Run this script from an elevated Administrator PowerShell window.'
    }
    throw
}

try {
    $startWatcher.Start()
    $stopWatcher.Start()
    $deadline = [DateTime]::UtcNow.AddSeconds($DurationSec)
    while ([DateTime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 100
    }
}
finally {
    $startWatcher.Stop()
    $stopWatcher.Stop()
    Start-Sleep -Milliseconds 250
    Unregister-Event -SubscriptionId $startSubscription.Id -ErrorAction SilentlyContinue
    Unregister-Event -SubscriptionId $stopSubscription.Id -ErrorAction SilentlyContinue
    $startWatcher.Dispose()
    $stopWatcher.Dispose()
}

$recordsArray = @($records.ToArray() | Sort-Object ObservedAt)
$callbackErrorsArray = @($callbackErrors.ToArray() | Sort-Object ObservedAt)
$probeFinishedAt = (Get-Date).ToString('o')

$lines = [System.Collections.Generic.List[string]]::new()
$lines.Add('OpenBitFun process event probe')
$lines.Add("probe_started_at=$(Format-LogValue $probeStartedAt)")
$lines.Add("probe_finished_at=$(Format-LogValue $probeFinishedAt)")
$lines.Add("probe_pid=$probeProcessId")
$lines.Add("openbitfun_pids=$(Format-LogValue (($rootProcessIds | ForEach-Object { [string]$_ }) -join ','))")
$lines.Add("duration_sec=$DurationSec")
$lines.Add("elevated=$isElevated")
$lines.Add("powershell_version=$(Format-LogValue $PSVersionTable.PSVersion.ToString())")
$lines.Add('event_source="Win32_ProcessStartTrace/Win32_ProcessStopTrace"')
$lines.Add("record_count=$(@($recordsArray).Count)")
$lines.Add("callback_error_count=$(@($callbackErrorsArray).Count)")
$lines.Add('')
$lines.Add('[callback_errors]')
if (@($callbackErrorsArray).Count -eq 0) {
    $lines.Add('none')
} else {
    foreach ($callbackError in $callbackErrorsArray) {
        $lines.Add(
            "observed_at=$(Format-LogValue $callbackError.ObservedAt) " +
            "event=$($callbackError.EventType) " +
            "message=$(Format-LogValue $callbackError.Message)"
        )
    }
}
$lines.Add('')
$lines.Add('[process_events]')
if (@($recordsArray).Count -eq 0) {
    $lines.Add('none')
} else {
    foreach ($record in $recordsArray) {
        $lineage = if (@($record.Lineage).Count -gt 0) {
            @($record.Lineage) -join ' > '
        } else {
            $null
        }
        $lines.Add(
            "observed_at=$(Format-LogValue $record.ObservedAt) " +
            "event=$($record.EventType) " +
            "name=$(Format-LogValue $record.ProcessName) " +
            "pid=$($record.ProcessId) " +
            "ppid=$($record.ParentProcessId) " +
            "attribution=$($record.AttributionStatus) " +
            "is_openbitfun=$($record.IsOpenBitFunDescendant) " +
            "lineage_complete=$($record.LineageComplete) " +
            "path=$(Format-LogValue $record.ExecutablePath) " +
            "command_line=$(Format-LogValue $record.CommandLine) " +
            "lineage=$(Format-LogValue $lineage)"
        )
    }
}

$textOutput = $lines -join [Environment]::NewLine
if ($OutputPath) {
    $textOutput | Set-Content -LiteralPath $OutputPath -Encoding utf8
}

$textOutput
