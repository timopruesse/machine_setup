
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
            cand -l 'Set log level'
            cand --level 'Set log level'
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -V 'Print version information'
            cand --version 'Print version information'
            cand -s 'Select a task to run'
            cand --select 'Select a task to run'
            cand -d 'Add debug information'
            cand --debug 'Add debug information'
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
            cand -l 'Set log level'
            cand --level 'Set log level'
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -V 'Print version information'
            cand --version 'Print version information'
            cand -s 'Select a task to run'
            cand --select 'Select a task to run'
            cand -d 'Add debug information'
            cand --debug 'Add debug information'
        }
        &'machine_setup;update'= {
            cand -c 'path to the config file'
            cand --config 'path to the config file'
            cand -t 'run a single task'
            cand --task 'run a single task'
            cand -l 'Set log level'
            cand --level 'Set log level'
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -V 'Print version information'
            cand --version 'Print version information'
            cand -s 'Select a task to run'
            cand --select 'Select a task to run'
            cand -d 'Add debug information'
            cand --debug 'Add debug information'
        }
        &'machine_setup;uninstall'= {
            cand -c 'path to the config file'
            cand --config 'path to the config file'
            cand -t 'run a single task'
            cand --task 'run a single task'
            cand -l 'Set log level'
            cand --level 'Set log level'
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -V 'Print version information'
            cand --version 'Print version information'
            cand -s 'Select a task to run'
            cand --select 'Select a task to run'
            cand -d 'Add debug information'
            cand --debug 'Add debug information'
        }
        &'machine_setup;list'= {
            cand -c 'path to the config file'
            cand --config 'path to the config file'
            cand -t 'run a single task'
            cand --task 'run a single task'
            cand -l 'Set log level'
            cand --level 'Set log level'
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -V 'Print version information'
            cand --version 'Print version information'
            cand -s 'Select a task to run'
            cand --select 'Select a task to run'
            cand -d 'Add debug information'
            cand --debug 'Add debug information'
        }
        &'machine_setup;help'= {
            cand -c 'path to the config file'
            cand --config 'path to the config file'
            cand -t 'run a single task'
            cand --task 'run a single task'
            cand -l 'Set log level'
            cand --level 'Set log level'
            cand -s 'Select a task to run'
            cand --select 'Select a task to run'
            cand -d 'Add debug information'
            cand --debug 'Add debug information'
        }
    ]
    $completions[$command]
}
