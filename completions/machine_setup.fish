complete -c machine_setup -n "__fish_use_subcommand" -s c -l config -d 'path to the config file' -r
complete -c machine_setup -n "__fish_use_subcommand" -s t -l task -d 'run a single task' -r
complete -c machine_setup -n "__fish_use_subcommand" -s l -l level -d 'set log level' -r
complete -c machine_setup -n "__fish_use_subcommand" -s s -l select -d 'select a task to run'
complete -c machine_setup -n "__fish_use_subcommand" -s d -l debug -d 'add debug information'
complete -c machine_setup -n "__fish_use_subcommand" -s f -l force -d 'force installation/uninstallation'
complete -c machine_setup -n "__fish_use_subcommand" -s h -l help -d 'Print help'
complete -c machine_setup -n "__fish_use_subcommand" -s V -l version -d 'Print version'
complete -c machine_setup -n "__fish_use_subcommand" -f -a "install" -d 'Install all of the defined tasks'
complete -c machine_setup -n "__fish_use_subcommand" -f -a "update" -d 'Update all of the defined tasks'
complete -c machine_setup -n "__fish_use_subcommand" -f -a "uninstall" -d 'Uninstall all of the defined tasks'
complete -c machine_setup -n "__fish_use_subcommand" -f -a "list" -d 'List defined tasks'
complete -c machine_setup -n "__fish_use_subcommand" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c machine_setup -n "__fish_seen_subcommand_from install" -s c -l config -d 'path to the config file' -r
complete -c machine_setup -n "__fish_seen_subcommand_from install" -s t -l task -d 'run a single task' -r
complete -c machine_setup -n "__fish_seen_subcommand_from install" -s l -l level -d 'set log level' -r
complete -c machine_setup -n "__fish_seen_subcommand_from install" -s s -l select -d 'select a task to run'
complete -c machine_setup -n "__fish_seen_subcommand_from install" -s d -l debug -d 'add debug information'
complete -c machine_setup -n "__fish_seen_subcommand_from install" -s f -l force -d 'force installation/uninstallation'
complete -c machine_setup -n "__fish_seen_subcommand_from install" -s h -l help -d 'Print help'
complete -c machine_setup -n "__fish_seen_subcommand_from install" -s V -l version -d 'Print version'
complete -c machine_setup -n "__fish_seen_subcommand_from update" -s c -l config -d 'path to the config file' -r
complete -c machine_setup -n "__fish_seen_subcommand_from update" -s t -l task -d 'run a single task' -r
complete -c machine_setup -n "__fish_seen_subcommand_from update" -s l -l level -d 'set log level' -r
complete -c machine_setup -n "__fish_seen_subcommand_from update" -s s -l select -d 'select a task to run'
complete -c machine_setup -n "__fish_seen_subcommand_from update" -s d -l debug -d 'add debug information'
complete -c machine_setup -n "__fish_seen_subcommand_from update" -s f -l force -d 'force installation/uninstallation'
complete -c machine_setup -n "__fish_seen_subcommand_from update" -s h -l help -d 'Print help'
complete -c machine_setup -n "__fish_seen_subcommand_from update" -s V -l version -d 'Print version'
complete -c machine_setup -n "__fish_seen_subcommand_from uninstall" -s c -l config -d 'path to the config file' -r
complete -c machine_setup -n "__fish_seen_subcommand_from uninstall" -s t -l task -d 'run a single task' -r
complete -c machine_setup -n "__fish_seen_subcommand_from uninstall" -s l -l level -d 'set log level' -r
complete -c machine_setup -n "__fish_seen_subcommand_from uninstall" -s s -l select -d 'select a task to run'
complete -c machine_setup -n "__fish_seen_subcommand_from uninstall" -s d -l debug -d 'add debug information'
complete -c machine_setup -n "__fish_seen_subcommand_from uninstall" -s f -l force -d 'force installation/uninstallation'
complete -c machine_setup -n "__fish_seen_subcommand_from uninstall" -s h -l help -d 'Print help'
complete -c machine_setup -n "__fish_seen_subcommand_from uninstall" -s V -l version -d 'Print version'
complete -c machine_setup -n "__fish_seen_subcommand_from list" -s c -l config -d 'path to the config file' -r
complete -c machine_setup -n "__fish_seen_subcommand_from list" -s t -l task -d 'run a single task' -r
complete -c machine_setup -n "__fish_seen_subcommand_from list" -s l -l level -d 'set log level' -r
complete -c machine_setup -n "__fish_seen_subcommand_from list" -s s -l select -d 'select a task to run'
complete -c machine_setup -n "__fish_seen_subcommand_from list" -s d -l debug -d 'add debug information'
complete -c machine_setup -n "__fish_seen_subcommand_from list" -s f -l force -d 'force installation/uninstallation'
complete -c machine_setup -n "__fish_seen_subcommand_from list" -s h -l help -d 'Print help'
complete -c machine_setup -n "__fish_seen_subcommand_from list" -s V -l version -d 'Print version'
complete -c machine_setup -n "__fish_seen_subcommand_from help; and not __fish_seen_subcommand_from install; and not __fish_seen_subcommand_from update; and not __fish_seen_subcommand_from uninstall; and not __fish_seen_subcommand_from list; and not __fish_seen_subcommand_from help" -f -a "install" -d 'Install all of the defined tasks'
complete -c machine_setup -n "__fish_seen_subcommand_from help; and not __fish_seen_subcommand_from install; and not __fish_seen_subcommand_from update; and not __fish_seen_subcommand_from uninstall; and not __fish_seen_subcommand_from list; and not __fish_seen_subcommand_from help" -f -a "update" -d 'Update all of the defined tasks'
complete -c machine_setup -n "__fish_seen_subcommand_from help; and not __fish_seen_subcommand_from install; and not __fish_seen_subcommand_from update; and not __fish_seen_subcommand_from uninstall; and not __fish_seen_subcommand_from list; and not __fish_seen_subcommand_from help" -f -a "uninstall" -d 'Uninstall all of the defined tasks'
complete -c machine_setup -n "__fish_seen_subcommand_from help; and not __fish_seen_subcommand_from install; and not __fish_seen_subcommand_from update; and not __fish_seen_subcommand_from uninstall; and not __fish_seen_subcommand_from list; and not __fish_seen_subcommand_from help" -f -a "list" -d 'List defined tasks'
complete -c machine_setup -n "__fish_seen_subcommand_from help; and not __fish_seen_subcommand_from install; and not __fish_seen_subcommand_from update; and not __fish_seen_subcommand_from uninstall; and not __fish_seen_subcommand_from list; and not __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
