# ---------------------------------------------------------------------------------------------
#   Shell Integration for PowerShell
# ---------------------------------------------------------------------------------------------

# Prevent installing more than once per session
if ((Test-Path variable:global:__TerminalState) -and $null -ne $Global:__TerminalState.OriginalPrompt) {
	return;
}

# Disable shell integration when the language mode is restricted
if ($ExecutionContext.SessionState.LanguageMode -ne "FullLanguage") {
	return;
}

$Global:__TerminalState = @{
	OriginalPrompt = $function:Prompt
	LastHistoryId = -1
	IsInExecution = $false
	Nonce = $null
	IsWindows10 = $false
	# Track $LASTEXITCODE before command execution to detect changes
	LastExitCodeBeforeCommand = $null
}

# Store the nonce
$Global:__TerminalState.Nonce = $env:TERMINAL_NONCE
$env:TERMINAL_NONCE = $null

$osVersion = [System.Environment]::OSVersion.Version
$Global:__TerminalState.IsWindows10 = $IsWindows -and $osVersion.Major -eq 10 -and $osVersion.Minor -eq 0 -and $osVersion.Build -lt 22000
Remove-Variable -Name osVersion -ErrorAction SilentlyContinue

# Escape value for OSC sequences
function Global:__Terminal-Escape-Value([string]$value) {
	[regex]::Replace($value, "[$([char]0x00)-$([char]0x1f)\\\n;]", { param($match)
			-Join (
				[System.Text.Encoding]::UTF8.GetBytes($match.Value) | ForEach-Object { '\x{0:x2}' -f $_ }
			)
		})
}

# Get the real exit code from the last command
# Strategy:
#   1. For external programs (.exe, scripts): use $LASTEXITCODE (provides real exit code like 130 for SIGINT)
#   2. For PowerShell cmdlets: $LASTEXITCODE is not set, so we use $? (boolean) -> 0 or 1
#   3. If $LASTEXITCODE changed during command execution, prefer that value
function Global:__Terminal-Get-ExitCode {
	param(
		[bool]$CommandSuccess,
		[object]$LastExitCodeBefore,
		[object]$LastExitCodeAfter
	)
	
	# Check if $LASTEXITCODE was set/changed by an external program
	# Note: $LASTEXITCODE persists across commands until another external program runs
	if ($null -ne $LastExitCodeAfter) {
		# If $LASTEXITCODE changed (or was set for the first time), use the new value
		if ($LastExitCodeBefore -ne $LastExitCodeAfter) {
			return $LastExitCodeAfter
		}
		# If command failed but $LASTEXITCODE didn't change, it might be a cmdlet failure
		# In this case, if $LASTEXITCODE is non-zero, it's from a previous external command
		if (-not $CommandSuccess -and $LastExitCodeAfter -eq 0) {
			# Cmdlet failed but last external program succeeded
			return 1
		}
		if (-not $CommandSuccess -and $LastExitCodeAfter -ne 0) {
			# Could be cmdlet failure after a failed external program
			# We can't distinguish, so return $LASTEXITCODE
			return $LastExitCodeAfter
		}
		# Command succeeded
		if ($CommandSuccess) {
			return 0
		}
	}
	
	# $LASTEXITCODE is null (no external program has run yet)
	# Use $? to determine success/failure
	return [int](-not $CommandSuccess)
}

function Global:Prompt() {
	# Capture these values IMMEDIATELY before any other code runs
	$CommandSuccess = $global:?
	$CurrentLastExitCode = $global:LASTEXITCODE
	
	Set-StrictMode -Off
	$LastHistoryEntry = Get-History -Count 1
	$Result = ""
	
	# Calculate the real exit code
	$ExitCode = __Terminal-Get-ExitCode -CommandSuccess $CommandSuccess `
		-LastExitCodeBefore $Global:__TerminalState.LastExitCodeBeforeCommand `
		-LastExitCodeAfter $CurrentLastExitCode
	
	# Finish previous command if applicable
	if ($Global:__TerminalState.LastHistoryId -ne -1 -and ($Global:__TerminalState.HasPSReadLine -eq $false -or $Global:__TerminalState.IsInExecution -eq $true)) {
		$Global:__TerminalState.IsInExecution = $false
		if ($LastHistoryEntry.Id -eq $Global:__TerminalState.LastHistoryId) {
			# No command was run (e.g., empty enter, ctrl+c before execution)
			$Result += "$([char]0x1b)]633;D`a"
		}
		else {
			# Command finished with exit code - now using real exit code
			$Result += "$([char]0x1b)]633;D;$ExitCode`a"
		}
	}
	
	# Prompt started (A)
	$Result += "$([char]0x1b)]633;A`a"
	
	# Current working directory
	$Result += if ($pwd.Provider.Name -eq 'FileSystem') { "$([char]0x1b)]633;P;Cwd=$(__Terminal-Escape-Value $pwd.ProviderPath)`a" }

	# Restore exit code state for original prompt and subsequent commands
	# This ensures $? and $LASTEXITCODE behave correctly for the user
	if ($ExitCode -ne 0) {
		Write-Error "failure" -ea ignore
	}
	$OriginalPrompt += $Global:__TerminalState.OriginalPrompt.Invoke()
	$Result += $OriginalPrompt

	# Command input started (B)
	$Result += "$([char]0x1b)]633;B`a"
	$Global:__TerminalState.LastHistoryId = $LastHistoryEntry.Id
	
	# Record current $LASTEXITCODE for next command's comparison
	$Global:__TerminalState.LastExitCodeBeforeCommand = $global:LASTEXITCODE
	
	return $Result
}

# Set IsWindows property
if ($PSVersionTable.PSVersion -lt "6.0") {
	[Console]::Write("$([char]0x1b)]633;P;IsWindows=$true`a")
}
else {
	[Console]::Write("$([char]0x1b)]633;P;IsWindows=$IsWindows`a")
}

# Handle PSReadLine for rich command detection
$Global:__TerminalState.HasPSReadLine = $false
if (Get-Module -Name PSReadLine) {
	$Global:__TerminalState.HasPSReadLine = $true
	[Console]::Write("$([char]0x1b)]633;P;HasRichCommandDetection=True`a")

	$Global:__TerminalState.OriginalPSConsoleHostReadLine = $function:PSConsoleHostReadLine
	function Global:PSConsoleHostReadLine {
		# Record $LASTEXITCODE BEFORE command execution
		$Global:__TerminalState.LastExitCodeBeforeCommand = $global:LASTEXITCODE
		
		$CommandLine = $Global:__TerminalState.OriginalPSConsoleHostReadLine.Invoke()
		$Global:__TerminalState.IsInExecution = $true

		# Command line (E)
		$Result = "$([char]0x1b)]633;E;"
		$Result += $(__Terminal-Escape-Value $CommandLine)
		if ($Global:__TerminalState.IsWindows10 -eq $false) {
			$Result += ";$($Global:__TerminalState.Nonce)"
		}
		$Result += "`a"

		# Command executed (C)
		$Result += "$([char]0x1b)]633;C`a"

		[Console]::Write($Result)

		$CommandLine
	}

	# Set ContinuationPrompt property
	$Global:__TerminalState.ContinuationPrompt = (Get-PSReadLineOption).ContinuationPrompt
	if ($Global:__TerminalState.ContinuationPrompt) {
		[Console]::Write("$([char]0x1b)]633;P;ContinuationPrompt=$(__Terminal-Escape-Value $Global:__TerminalState.ContinuationPrompt)`a")
	}

	# For programmatic terminals (bash_tool), disable PSReadLine inline
	# prediction to prevent ConPTY rendering interference. ConPTY's async
	# renderer can flush prediction rendering (cursor repositioning, partial
	# text fragments) AFTER the 633;C marker, polluting captured output.
	if ($env:BITFUN_NONINTERACTIVE -eq "1") {
		try { Set-PSReadLineOption -PredictionSource None } catch {}
	}
}

