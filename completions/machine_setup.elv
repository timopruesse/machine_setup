
use builtin;
use str;

set edit:completion:arg-completer[machine_setup] = {|@words|
    fn spaces {|n|
        builtin:repeat $n ' ' | str:join ''
    }
    fn cand {|text desc|
        edit:complex-candidate $text &display=$text' '(spaces (- 14 (wcswidth $text)))$desc
    }
    var command = 'machine_setup'
    for word $words[1..-1] {
        if (str:has-prefix $word '-') {
            break
        }
        set command = $command';'$word
    }
    var completions = [
        &'machine_setup'= {
            cand -c 'path to the config file'
            cand --config 'path to the config file'
            cand -t 'run a single task'
            cand --task 'run a single task'
            cand -l 'set log level'
            cand --level 'set log level'
            cand -s 'select a task to run'
            cand --select 'select a task to run'
            cand -d 'add debug information'
            cand --debug 'add debug information'
            cand -f 'force installation/uninstallation'
            cand --force 'force installation/uninstallation'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
            cand install 'Install all of the defined tasks'
            cand update 'Update all of the defined tasks'
            cand uninstall 'Uninstall all of the defined tasks'
            cand list 'List defined tasks'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'machine_setup;install'= {
            cand -c 'path to the config file'
            cand --config 'path to the config file'
            cand -t 'run a single task'
            cand --task 'run a single task'
            cand -l 'set log level'
            cand --level 'set log level'
            cand -s 'select a task to run'
            cand --select 'select a task to run'
            cand -d 'add debug information'
            cand --debug 'add debug information'
            cand -f 'force installation/uninstallation'
            cand --force 'force installation/uninstallation'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'machine_setup;update'= {
            cand -c 'path to the config file'
            cand --config 'path to the config file'
            cand -t 'run a single task'
            cand --task 'run a single task'
            cand -l 'set log level'
            cand --level 'set log level'
            cand -s 'select a task to run'
            cand --select 'select a task to run'
            cand -d 'add debug information'
            cand --debug 'add debug information'
            cand -f 'force installation/uninstallation'
            cand --force 'force installation/uninstallation'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'machine_setup;uninstall'= {
            cand -c 'path to the config file'
            cand --config 'path to the config file'
            cand -t 'run a single task'
            cand --task 'run a single task'
            cand -l 'set log level'
            cand --level 'set log level'
            cand -s 'select a task to run'
            cand --select 'select a task to run'
            cand -d 'add debug information'
            cand --debug 'add debug information'
            cand -f 'force installation/uninstallation'
            cand --force 'force installation/uninstallation'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'machine_setup;list'= {
            cand -c 'path to the config file'
            cand --config 'path to the config file'
            cand -t 'run a single task'
            cand --task 'run a single task'
            cand -l 'set log level'
            cand --level 'set log level'
            cand -s 'select a task to run'
            cand --select 'select a task to run'
            cand -d 'add debug information'
            cand --debug 'add debug information'
            cand -f 'force installation/uninstallation'
            cand --force 'force installation/uninstallation'
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
        }
        &'machine_setup;help'= {
            cand install 'Install all of the defined tasks'
            cand update 'Update all of the defined tasks'
            cand uninstall 'Uninstall all of the defined tasks'
            cand list 'List defined tasks'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'machine_setup;help;install'= {
        }
        &'machine_setup;help;update'= {
        }
        &'machine_setup;help;uninstall'= {
        }
        &'machine_setup;help;list'= {
        }
        &'machine_setup;help;help'= {
        }
    ]
    $completions[$command]
}
