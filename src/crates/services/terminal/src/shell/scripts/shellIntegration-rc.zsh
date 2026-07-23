# ---------------------------------------------------------------------------------------------
#   Shell Integration for Zsh
# ---------------------------------------------------------------------------------------------
builtin autoload -Uz add-zsh-hook is-at-least

# Prevent the script recursing when setting up
if [ -n "$TERMINAL_SHELL_INTEGRATION" ]; then
	ZDOTDIR=$USER_ZDOTDIR
	builtin return
fi

TERMINAL_SHELL_INTEGRATION=1

# Fix up ZDOTDIR if shell integration was injected
if [[ "$TERMINAL_INJECTION" == "1" ]]; then
	if [[ $options[norcs] = off && -f $USER_ZDOTDIR/.zshrc ]]; then
		TERMINAL_ZDOTDIR=$ZDOTDIR
		ZDOTDIR=$USER_ZDOTDIR
		. $USER_ZDOTDIR/.zshrc
	fi
fi

# Shell integration was disabled, exit
if [ -z "$TERMINAL_SHELL_INTEGRATION" ]; then
	builtin return
fi

# Escape value for OSC sequences
__tsi_escape_value() {
	builtin emulate -L zsh
	builtin local LC_ALL=C str="$1" i byte token out='' val

	for (( i = 0; i < ${#str}; ++i )); do
		byte="${str:$i:1}"
		val=$(printf "%d" "'$byte")
		if (( val < 31 )); then
			token=$(printf "\\\\x%02x" "'$byte")
		elif [ "$byte" = "\\" ]; then
			token="\\\\"
		elif [ "$byte" = ";" ]; then
			token="\\x3b"
		else
			token="$byte"
		fi
		out+="$token"
	done

	builtin print -r -- "$out"
}

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
	builtin printf '\e]633;P;Cwd=%s\a' "$(__tsi_escape_value "${PWD}")"
}

__tsi_command_output_start() {
	builtin printf '\e]633;E;%s;%s\a' "$(__tsi_escape_value "${__tsi_current_command}")" "$__tsi_nonce"
	builtin printf '\e]633;C\a'
}

__tsi_continuation_start() {
	builtin printf '\e]633;F\a'
}

__tsi_continuation_end() {
	builtin printf '\e]633;G\a'
}

__tsi_right_prompt_start() {
	builtin printf '\e]633;H\a'
}

__tsi_right_prompt_end() {
	builtin printf '\e]633;I\a'
}

__tsi_command_complete() {
	if [[ "$__tsi_current_command" == "" ]]; then
		builtin printf '\e]633;D\a'
	else
		builtin printf '\e]633;D;%s\a' "$__tsi_status"
	fi
	__tsi_update_cwd
}

if [[ -o NOUNSET ]]; then
	if [ -z "${RPROMPT-}" ]; then
		RPROMPT=""
	fi
fi

__tsi_update_prompt() {
	__tsi_prior_prompt="$PS1"
	__tsi_prior_prompt2="$PS2"
	__tsi_in_command_execution=""
	PS1="%{$(__tsi_prompt_start)%}$PS1%{$(__tsi_prompt_end)%}"
	PS2="%{$(__tsi_continuation_start)%}$PS2%{$(__tsi_continuation_end)%}"
	if [ -n "$RPROMPT" ]; then
		__tsi_prior_rprompt="$RPROMPT"
		RPROMPT="%{$(__tsi_right_prompt_start)%}$RPROMPT%{$(__tsi_right_prompt_end)%}"
	fi
}

__tsi_precmd() {
	builtin local __tsi_status="$?"
	if [ -z "${__tsi_in_command_execution-}" ]; then
		__tsi_command_output_start
	fi

	__tsi_command_complete "$__tsi_status"
	__tsi_current_command=""

	if [ -n "$__tsi_in_command_execution" ]; then
		__tsi_update_prompt
	fi
}

__tsi_preexec() {
	PS1="$__tsi_prior_prompt"
	PS2="$__tsi_prior_prompt2"
	if [ -n "$RPROMPT" ]; then
		RPROMPT="$__tsi_prior_rprompt"
	fi
	__tsi_in_command_execution="1"
	__tsi_current_command=$1
	__tsi_command_output_start
}

add-zsh-hook precmd __tsi_precmd
add-zsh-hook preexec __tsi_preexec

if [[ $options[login] = off && $USER_ZDOTDIR != $TERMINAL_ZDOTDIR ]]; then
	ZDOTDIR=$USER_ZDOTDIR
fi

