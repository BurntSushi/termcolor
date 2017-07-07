#!/bin/sh

##
# Compares options in `rg --help` output to options in zsh completion function
#
# @todo If we could rely on zsh being installed we could change all of this to
# simply source the completion-function file and pull the rg_args array out...

set -e

main() {
    local rg="target/${TARGET}/release/rg"
    local _rg='complete/_rg'
    local ret='0'
    local helpTemp="$( mktemp )"
    local compTemp="$( mktemp )"
    local diff

    [ -e "${rg}" ] || rg="target/${TARGET}/debug/rg"

    if [ ! -e "${rg}" ]; then
        printf 'File not found: %s\n' "${rg}" >&2
        ret='1'
    elif [ ! -e "${_rg}" ]; then
        printf 'File not found: %s\n' "${_rg}" >&2
        ret='1'
    else
        # 'Parse' options out of the `--help` output. To prevent false positives
        # we only look at lines where the first non-white-space character is `-`
        "${rg}" --help |
        "${rg}" -- '^\s*-' |
        "${rg}" -io -- '[\t ,](-[a-z0-9]|--[a-z0-9-]+)\b' |
        tr -d '\t ,' |
        sort -u > "${helpTemp}"

        # 'Parse' options out of the completion-function file. To prevent false
        # negatives, we:
        #
        # * Exclude lines that don't start with punctuation expected of option
        #   definitions
        # * Exclude lines that don't appear to have a bracketed description
        #   suitable for `_arguments`
        # * Exclude those bracketed descriptions so we don't match options
        #   which might be referenced in them
        # * Exclude parenthetical lists of exclusive options so we don't match
        #   those
        #
        # This does of course make the following assumptions:
        #
        # * Each option definition is on its own (single) line
        # * Each option definition has a description
        # * Option names are static — i.e., they aren't constructed from
        #   variables or command substitutions. Brace expansion is OK as long as
        #   each component of the expression is a complete option flag — in
        #   other words, `{--foo,--bar}` is valid, but `--{foo,bar}` is not
        # * Bracketed descriptions must contain at least two characters and must
        #   not begin with `!`, `@`, or `^` (in order to avoid confusion with
        #   shell syntax)
        "${rg}" -- "^\s*[\"':({*-]" "${_rg}" |
        "${rg}" --replace '$1' -- '^.*?(?:\(.+?\).*?)?(-.+)\[[^!@^].+\].*' |
        tr -d "\t (){}*=+:'\"" |
        tr ',' '\n' |
        sort -u > "${compTemp}"

        diff="$(
            if diff --help 2>&1 | grep -qF -- '--label'; then
                diff -U2 \
                    --label '`rg --help`' \
                    --label "${_rg}" \
                    "${helpTemp}" "${compTemp}" || true
            else
                diff -U2 \
                    -L '`rg --help`' \
                    -L "${_rg}" \
                    "${helpTemp}" "${compTemp}" || true
            fi
        )"

        [ -n "${diff}" ] && {
            printf '%s\n' 'zsh completion options differ from `--help` options:' >&2
            printf '%s\n' "${diff}" >&2
            ret='1'
        }
    fi

    rm -f "${helpTemp}" "${compTemp}"

    return "${ret}"
}

main "${@}"
