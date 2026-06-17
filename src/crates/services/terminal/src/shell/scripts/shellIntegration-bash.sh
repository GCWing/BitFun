# ---------------------------------------------------------------------------------------------
#   Shell Integration for Bash
# ---------------------------------------------------------------------------------------------

# Prevent the script recursing when setting up
if [[ -n "${TERMINAL_SHELL_INTEGRATION:-}" ]]; then
	builtin return
fi

TERMINAL_SHELL_INTEGRATION=1

# Run relevant rc/profile only if shell integration has been injected
# NOTE: If user config contains 'exec', 'exit', or 'return', shell integration
# will fail and the application will report a timeout error.
if [ "$TERMINAL_INJECTION" == "1" ]; then
	if [ -z "$TERMINAL_SHELL_LOGIN" ]; then
		# Non-login shell: source .bashrc
		[ -r ~/.bashrc ] && . ~/.bashrc
	else
		# Login shell: imitate -l because --init-file doesn't support it
		# Source /etc/profile first (system-wide)
		[ -r /etc/profile ] && . /etc/profile
		# Then source the first user profile that exists
		if [ -r ~/.bash_profile ]; then
			. ~/.bash_profile
		elif [ -r ~/.bash_login ]; then
			. ~/.bash_login
		elif [ -r ~/.profile ]; then
			. ~/.profile
		fi
		builtin unset TERMINAL_SHELL_LOGIN
	fi
	builtin unset TERMINAL_INJECTION
fi

# If we reach here, user config didn't interrupt us
# Continue with shell integration setup

if [ -z "$TERMINAL_SHELL_INTEGRATION" ]; then
	builtin return
fi

# Escape value for OSC sequences
__tsi_escape_value() {
	builtin local LC_ALL=C out
	out=${1//\\/\\\\}
	out=${out//;/\\x3b}
	builtin printf '%s\n' "${out}"
}

# Detect Windows environment
__tsi_regex_environment="^CYGWIN*|MINGW*|MSYS*"
if [[ "$(uname -s)" =~ $__tsi_regex_environment ]]; then
	builtin printf '\e]633;P;IsWindows=True\a'
	__tsi_is_windows=1
else
	__tsi_is_windows=0
fi

# History verification
__tsi_regex_histcontrol=".*(erasedups|ignoreboth|ignoredups).*"
if [[ "${HISTCONTROL:-}" =~ $__tsi_regex_histcontrol ]]; then
	__tsi_history_verify=0
else
	__tsi_history_verify=1
fi

builtin unset __tsi_regex_environment
builtin unset __tsi_regex_histcontrol

__tsi_initialized=0
__tsi_original_PS1="$PS1"
__tsi_original_PS2="$PS2"
__tsi_custom_PS1=""
__tsi_custom_PS2=""
__tsi_in_command_execution="1"
__tsi_current_command=""

# Nonce for command verification
__tsi_nonce="${TERMINAL_NONCE:-}"
unset TERMINAL_NONCE

# Report rich command detection support
builtin printf '\e]633;P;HasRichCommandDetection=True\a'

__tsi_prompt_start() {
	builtin printf '\e]633;A\a'
}

__tsi_prompt_end() {
	builtin printf '\e]633;B\a'
}

__tsi_update_cwd() {
	if [ "$__tsi_is_windows" = "1" ]; then
		__tsi_cwd="$(cygpath -m "$PWD")"
	else
		__tsi_cwd="$PWD"
	fi
	builtin printf '\e]633;P;Cwd=%s\a' "$(__tsi_escape_value "$__tsi_cwd")"
}

__tsi_command_output_start() {
	if [[ -z "${__tsi_first_prompt-}" ]]; then
		builtin return
	fi
	builtin printf '\e]633;E;%s;%s\a' "$(__tsi_escape_value "${__tsi_current_command}")" "$__tsi_nonce"
	builtin printf '\e]633;C\a'
}

__tsi_continuation_start() {
	builtin printf '\e]633;F\a'
}

__tsi_continuation_end() {
	builtin printf '\e]633;G\a'
}

__tsi_command_complete() {
	if [[ -z "${__tsi_first_prompt-}" ]]; then
		__tsi_update_cwd
		builtin return
	fi
	if [ "$__tsi_current_command" = "" ]; then
		builtin printf '\e]633;D\a'
	else
		builtin printf '\e]633;D;%s\a' "$__tsi_status"
	fi
	__tsi_update_cwd
}

__tsi_update_prompt() {
	if [ "$__tsi_in_command_execution" = "1" ]; then
		if [[ "$__tsi_custom_PS1" == "" || "$__tsi_custom_PS1" != "$PS1" ]]; then
			__tsi_original_PS1=$PS1
			__tsi_custom_PS1="\[$(__tsi_prompt_start)\]$__tsi_original_PS1\[$(__tsi_prompt_end)\]"
			PS1="$__tsi_custom_PS1"
		fi
		if [[ "$__tsi_custom_PS2" == "" || "$__tsi_custom_PS2" != "$PS2" ]]; then
			__tsi_original_PS2=$PS2
			__tsi_custom_PS2="\[$(__tsi_continuation_start)\]$__tsi_original_PS2\[$(__tsi_continuation_end)\]"
			PS2="$__tsi_custom_PS2"
		fi
		__tsi_in_command_execution="0"
	fi
}

__tsi_precmd() {
	__tsi_command_complete "$__tsi_status"
	__tsi_current_command=""
	__tsi_first_prompt=1
	__tsi_update_prompt
}

__tsi_preexec() {
	__tsi_initialized=1
	if [[ ! $BASH_COMMAND == __tsi_prompt* ]]; then
		if [ "$__tsi_history_verify" = "1" ]; then
			__tsi_current_command="$(builtin history 1 | sed 's/ *[0-9]* *//')"
		else
			__tsi_current_command=$BASH_COMMAND
		fi
	else
		__tsi_current_command=""
	fi
	__tsi_command_output_start
}

# Set up DEBUG trap for preexec
__tsi_dbg_trap="$(trap -p DEBUG | sed "s/trap -- '\\(.*\\)' DEBUG/\\1/")"

if [[ -z "$__tsi_dbg_trap" ]]; then
	__tsi_preexec_only() {
		if [ "$__tsi_in_command_execution" = "0" ]; then
			__tsi_in_command_execution="1"
			__tsi_preexec
		fi
	}
	trap '__tsi_preexec_only "$_"' DEBUG
elif [[ "$__tsi_dbg_trap" != '__tsi_preexec "$_"' && "$__tsi_dbg_trap" != '__tsi_preexec_all "$_"' ]]; then
	__tsi_preexec_all() {
		if [ "$__tsi_in_command_execution" = "0" ]; then
			__tsi_in_command_execution="1"
			__tsi_preexec
			builtin eval "${__tsi_dbg_trap}"
		fi
	}
	trap '__tsi_preexec_all "$_"' DEBUG
fi

__tsi_update_prompt

__tsi_restore_exit_code() {
	return "$1"
}

__tsi_prompt_cmd_original() {
	__tsi_status="$?"
	builtin local cmd
	__tsi_restore_exit_code "${__tsi_status}"
	for cmd in "${__tsi_original_prompt_command[@]}"; do
		eval "${cmd:-}"
	done
	__tsi_precmd
}

__tsi_prompt_cmd() {
	__tsi_status="$?"
	__tsi_precmd
}

__tsi_original_prompt_command=${PROMPT_COMMAND:-}

if [[ -n "${__tsi_original_prompt_command:-}" && "${__tsi_original_prompt_command:-}" != "__tsi_prompt_cmd" ]]; then
	PROMPT_COMMAND=__tsi_prompt_cmd_original
else
	PROMPT_COMMAND=__tsi_prompt_cmd
fi
