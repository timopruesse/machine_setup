
using namespace System.Management.Automation
using namespace System.Management.Automation.Language

Register-ArgumentCompleter -Native -CommandName 'machine_setup' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    $command = @(
        'machine_setup'
        for ($i = 1; $i -lt $commandElements.Count; $i++) {
            $element = $commandElements[$i]
            if ($element -isnot [StringConstantExpressionAst] -or
                $element.StringConstantType -ne [StringConstantType]::BareWord -or
                $element.Value.StartsWith('-') -or
                $element.Value -eq $wordToComplete) {
                break
        }
        $element.Value
    }) -join ';'

    $completions = @(switch ($command) {
        'machine_setup' {
            [CompletionResult]::new('-c', 'c', [CompletionResultType]::ParameterName, 'path to the config file')
            [CompletionResult]::new('--config', 'config', [CompletionResultType]::ParameterName, 'path to the config file')
            [CompletionResult]::new('-t', 't', [CompletionResultType]::ParameterName, 'run a single task')
            [CompletionResult]::new('--task', 'task', [CompletionResultType]::ParameterName, 'run a single task')
            [CompletionResult]::new('-l', 'l', [CompletionResultType]::ParameterName, 'set log level')
            [CompletionResult]::new('--level', 'level', [CompletionResultType]::ParameterName, 'set log level')
            [CompletionResult]::new('-s', 's', [CompletionResultType]::ParameterName, 'select a task to run')
            [CompletionResult]::new('--select', 'select', [CompletionResultType]::ParameterName, 'select a task to run')
            [CompletionResult]::new('-d', 'd', [CompletionResultType]::ParameterName, 'add debug information')
            [CompletionResult]::new('--debug', 'debug', [CompletionResultType]::ParameterName, 'add debug information')
            [CompletionResult]::new('-f', 'f', [CompletionResultType]::ParameterName, 'force installation/uninstallation')
            [CompletionResult]::new('--force', 'force', [CompletionResultType]::ParameterName, 'force installation/uninstallation')
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('-V', 'V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', 'version', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('install', 'install', [CompletionResultType]::ParameterValue, 'Install all of the defined tasks')
            [CompletionResult]::new('update', 'update', [CompletionResultType]::ParameterValue, 'Update all of the defined tasks')
            [CompletionResult]::new('uninstall', 'uninstall', [CompletionResultType]::ParameterValue, 'Uninstall all of the defined tasks')
            [CompletionResult]::new('list', 'list', [CompletionResultType]::ParameterValue, 'List defined tasks')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'machine_setup;install' {
            [CompletionResult]::new('-c', 'c', [CompletionResultType]::ParameterName, 'path to the config file')
            [CompletionResult]::new('--config', 'config', [CompletionResultType]::ParameterName, 'path to the config file')
            [CompletionResult]::new('-t', 't', [CompletionResultType]::ParameterName, 'run a single task')
            [CompletionResult]::new('--task', 'task', [CompletionResultType]::ParameterName, 'run a single task')
            [CompletionResult]::new('-l', 'l', [CompletionResultType]::ParameterName, 'set log level')
            [CompletionResult]::new('--level', 'level', [CompletionResultType]::ParameterName, 'set log level')
            [CompletionResult]::new('-s', 's', [CompletionResultType]::ParameterName, 'select a task to run')
            [CompletionResult]::new('--select', 'select', [CompletionResultType]::ParameterName, 'select a task to run')
            [CompletionResult]::new('-d', 'd', [CompletionResultType]::ParameterName, 'add debug information')
            [CompletionResult]::new('--debug', 'debug', [CompletionResultType]::ParameterName, 'add debug information')
            [CompletionResult]::new('-f', 'f', [CompletionResultType]::ParameterName, 'force installation/uninstallation')
            [CompletionResult]::new('--force', 'force', [CompletionResultType]::ParameterName, 'force installation/uninstallation')
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('-V', 'V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', 'version', [CompletionResultType]::ParameterName, 'Print version')
            break
        }
        'machine_setup;update' {
            [CompletionResult]::new('-c', 'c', [CompletionResultType]::ParameterName, 'path to the config file')
            [CompletionResult]::new('--config', 'config', [CompletionResultType]::ParameterName, 'path to the config file')
            [CompletionResult]::new('-t', 't', [CompletionResultType]::ParameterName, 'run a single task')
            [CompletionResult]::new('--task', 'task', [CompletionResultType]::ParameterName, 'run a single task')
            [CompletionResult]::new('-l', 'l', [CompletionResultType]::ParameterName, 'set log level')
            [CompletionResult]::new('--level', 'level', [CompletionResultType]::ParameterName, 'set log level')
            [CompletionResult]::new('-s', 's', [CompletionResultType]::ParameterName, 'select a task to run')
            [CompletionResult]::new('--select', 'select', [CompletionResultType]::ParameterName, 'select a task to run')
            [CompletionResult]::new('-d', 'd', [CompletionResultType]::ParameterName, 'add debug information')
            [CompletionResult]::new('--debug', 'debug', [CompletionResultType]::ParameterName, 'add debug information')
            [CompletionResult]::new('-f', 'f', [CompletionResultType]::ParameterName, 'force installation/uninstallation')
            [CompletionResult]::new('--force', 'force', [CompletionResultType]::ParameterName, 'force installation/uninstallation')
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('-V', 'V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', 'version', [CompletionResultType]::ParameterName, 'Print version')
            break
        }
        'machine_setup;uninstall' {
            [CompletionResult]::new('-c', 'c', [CompletionResultType]::ParameterName, 'path to the config file')
            [CompletionResult]::new('--config', 'config', [CompletionResultType]::ParameterName, 'path to the config file')
            [CompletionResult]::new('-t', 't', [CompletionResultType]::ParameterName, 'run a single task')
            [CompletionResult]::new('--task', 'task', [CompletionResultType]::ParameterName, 'run a single task')
            [CompletionResult]::new('-l', 'l', [CompletionResultType]::ParameterName, 'set log level')
            [CompletionResult]::new('--level', 'level', [CompletionResultType]::ParameterName, 'set log level')
            [CompletionResult]::new('-s', 's', [CompletionResultType]::ParameterName, 'select a task to run')
            [CompletionResult]::new('--select', 'select', [CompletionResultType]::ParameterName, 'select a task to run')
            [CompletionResult]::new('-d', 'd', [CompletionResultType]::ParameterName, 'add debug information')
            [CompletionResult]::new('--debug', 'debug', [CompletionResultType]::ParameterName, 'add debug information')
            [CompletionResult]::new('-f', 'f', [CompletionResultType]::ParameterName, 'force installation/uninstallation')
            [CompletionResult]::new('--force', 'force', [CompletionResultType]::ParameterName, 'force installation/uninstallation')
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('-V', 'V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', 'version', [CompletionResultType]::ParameterName, 'Print version')
            break
        }
        'machine_setup;list' {
            [CompletionResult]::new('-c', 'c', [CompletionResultType]::ParameterName, 'path to the config file')
            [CompletionResult]::new('--config', 'config', [CompletionResultType]::ParameterName, 'path to the config file')
            [CompletionResult]::new('-t', 't', [CompletionResultType]::ParameterName, 'run a single task')
            [CompletionResult]::new('--task', 'task', [CompletionResultType]::ParameterName, 'run a single task')
            [CompletionResult]::new('-l', 'l', [CompletionResultType]::ParameterName, 'set log level')
            [CompletionResult]::new('--level', 'level', [CompletionResultType]::ParameterName, 'set log level')
            [CompletionResult]::new('-s', 's', [CompletionResultType]::ParameterName, 'select a task to run')
            [CompletionResult]::new('--select', 'select', [CompletionResultType]::ParameterName, 'select a task to run')
            [CompletionResult]::new('-d', 'd', [CompletionResultType]::ParameterName, 'add debug information')
            [CompletionResult]::new('--debug', 'debug', [CompletionResultType]::ParameterName, 'add debug information')
            [CompletionResult]::new('-f', 'f', [CompletionResultType]::ParameterName, 'force installation/uninstallation')
            [CompletionResult]::new('--force', 'force', [CompletionResultType]::ParameterName, 'force installation/uninstallation')
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help')
            [CompletionResult]::new('-V', 'V ', [CompletionResultType]::ParameterName, 'Print version')
            [CompletionResult]::new('--version', 'version', [CompletionResultType]::ParameterName, 'Print version')
            break
        }
        'machine_setup;help' {
            [CompletionResult]::new('install', 'install', [CompletionResultType]::ParameterValue, 'Install all of the defined tasks')
            [CompletionResult]::new('update', 'update', [CompletionResultType]::ParameterValue, 'Update all of the defined tasks')
            [CompletionResult]::new('uninstall', 'uninstall', [CompletionResultType]::ParameterValue, 'Uninstall all of the defined tasks')
            [CompletionResult]::new('list', 'list', [CompletionResultType]::ParameterValue, 'List defined tasks')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'machine_setup;help;install' {
            break
        }
        'machine_setup;help;update' {
            break
        }
        'machine_setup;help;uninstall' {
            break
        }
        'machine_setup;help;list' {
            break
        }
        'machine_setup;help;help' {
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}
