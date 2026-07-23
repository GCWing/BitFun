# ---------------------------------------------------------------------------------------------
#   Shell Integration for Fish
# ---------------------------------------------------------------------------------------------

# Don't run in scripts, other terminals, or more than once per session.
status is-interactive
and ! set --query TERMINAL_SHELL_INTEGRATION
or exit

set --global TERMINAL_SHELL_INTEGRATION 1

# Nonce for command verification
if set -q TERMINAL_NONCE
	set -g __tsi_nonce $TERMINAL_NONCE
	set -e TERMINAL_NONCE
end

# Helper function to emit OSC sequences
function __tsi_esc -d "Emit escape sequences for shell integration"
	builtin printf "\e]633;%s\a" (string join ";" -- $argv)
end

# Escape a value for OSC sequences
function __tsi_escape_value
	echo $argv \
	| string replace --all '\\' '\\\\' \
	| string replace --all ';' '\\x3b' \
	;
end

# Tracks if the shell has been initialized
set -g tsi_initialized 0

# Sent right before executing an interactive command
function __tsi_cmd_executed --on-event fish_preexec
	__tsi_esc E (__tsi_escape_value "$argv") $__tsi_nonce
	__tsi_esc C
	set --global _tsi_has_cmd
end

# Sent right after an interactive command has finished
function __tsi_cmd_finished --on-event fish_postexec
	__tsi_esc D $status
end

# Sent when a command line is cleared or reset
function __tsi_cmd_clear --on-event fish_cancel
	if test $tsi_initialized -eq 0;
		return
	end
	__tsi_esc E "" $__tsi_nonce
	__tsi_esc C
	__tsi_esc D
end

# Preserve the user's existing prompt
function __preserve_fish_prompt --on-event fish_prompt
	if functions --query fish_prompt
		if functions --query __tsi_fish_prompt
			functions --erase __tsi_fish_prompt
		end
		functions --copy fish_prompt __tsi_fish_prompt
		functions --erase __preserve_fish_prompt
		__init_terminal_shell_integration
	else
		if functions --query __tsi_fish_prompt
			functions --erase __preserve_fish_prompt
			__init_terminal_shell_integration
		else
			function __tsi_fish_prompt
				echo -n (whoami)@(prompt_hostname) (prompt_pwd) '~> '
			end
		end
	end
end

# Update current working directory
function __tsi_update_cwd --on-event fish_prompt
	__tsi_esc P Cwd=(__tsi_escape_value "$PWD")

	if set --query _tsi_has_cmd
		set --erase _tsi_has_cmd
	else
		__tsi_cmd_clear
	end
end

# Prompt start marker
function __tsi_fish_prompt_start
	__tsi_esc A
	set -g tsi_initialized 1
end

# Command input start marker
function __tsi_fish_cmd_start
	__tsi_esc B
end

function __tsi_fish_has_mode_prompt -d "Returns true if fish_mode_prompt is defined and not empty"
	functions fish_mode_prompt | string match -rvq '^ *(#|function |end$|$)'
end

# Initialize shell integration
function __init_terminal_shell_integration
	if __tsi_fish_has_mode_prompt
		functions --copy fish_mode_prompt __tsi_fish_mode_prompt

		function fish_mode_prompt
			__tsi_fish_prompt_start
			__tsi_fish_mode_prompt
		end

		function fish_prompt
			__tsi_fish_prompt
			__tsi_fish_cmd_start
		end
	else
		function fish_prompt
			__tsi_fish_prompt_start
			__tsi_fish_prompt
			__tsi_fish_cmd_start
		end
	end
end

# Report rich command detection support
__tsi_esc P HasRichCommandDetection=True

__preserve_fish_prompt

