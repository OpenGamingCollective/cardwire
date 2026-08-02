pub const FISH_COMPLETIONS: &str = r#"
# Cardwire fish completions

# Top-level subcommands
complete -c cardwire -f -n '__fish_use_subcommand' -a 'set' -d 'Set to the desired mode'
complete -c cardwire -f -n '__fish_use_subcommand' -a 'get' -d 'Get the current mode'
complete -c cardwire -f -n '__fish_use_subcommand' -a 'list' -d 'Print the gpu list'
complete -c cardwire -f -n '__fish_use_subcommand' -a 'gpu' -d 'Manage a specific GPU by its id'
complete -c cardwire -f -n '__fish_use_subcommand' -a 'config' -d 'Manage daemon configuration'
complete -c cardwire -f -n '__fish_use_subcommand' -a 'manager' -d 'Manager operations'
complete -c cardwire -f -n '__fish_use_subcommand' -a 'debug' -d 'Debug operations'

# set <mode>
complete -c cardwire -f -n '__fish_seen_subcommand_from set; and test (count (commandline -opc)) -eq 2' -a "integrated\t'Integrated GPU only' hybrid\t'Hybrid mode' manual\t'Manual mode' smart\t'Smart mode'"

# list
complete -c cardwire -f -n '__fish_seen_subcommand_from list' -l full -d 'Print the whole pci list'
complete -c cardwire -f -n '__fish_seen_subcommand_from list' -l json -d 'Print the gpu list in json format'

# gpu <id> --action
complete -c cardwire -f -n '__fish_seen_subcommand_from gpu; and test (count (commandline -opc)) -eq 2' -a '(cardwire complete-gpus 2>/dev/null)'

# Using -a instead of -l for gpu actions so they show up immediately on <TAB> without needing to type "-"
complete -c cardwire -f -n '__fish_seen_subcommand_from gpu; and test (count (commandline -opc)) -ge 3' -a " '--block'\t'Block a specific gpu' '--unblock'\t'Unblock a specific gpu' '--lsof'\t'List open files on the GPU' '--power'\t'Get GPU power state' "

# config <action>
complete -c cardwire -f -n '__fish_seen_subcommand_from config; and not __fish_seen_subcommand_from auto-apply-gpu-state experimental-nvidia-block battery-auto-switch battery-auto-switch-mode external-display-auto-switch save' -a 'auto-apply-gpu-state' -d 'Get or set AutoApplyGpuState'
complete -c cardwire -f -n '__fish_seen_subcommand_from config; and not __fish_seen_subcommand_from auto-apply-gpu-state experimental-nvidia-block battery-auto-switch battery-auto-switch-mode external-display-auto-switch save' -a 'experimental-nvidia-block' -d 'Get or set ExperimentalNvidiaBlock'
complete -c cardwire -f -n '__fish_seen_subcommand_from config; and not __fish_seen_subcommand_from auto-apply-gpu-state experimental-nvidia-block battery-auto-switch battery-auto-switch-mode external-display-auto-switch save' -a 'battery-auto-switch' -d 'Get or set BatteryAutoSwitch'
complete -c cardwire -f -n '__fish_seen_subcommand_from config; and not __fish_seen_subcommand_from auto-apply-gpu-state experimental-nvidia-block battery-auto-switch battery-auto-switch-mode external-display-auto-switch save' -a 'battery-auto-switch-mode' -d 'Get or set BatteryAutoSwitchMode'
complete -c cardwire -f -n '__fish_seen_subcommand_from config; and not __fish_seen_subcommand_from auto-apply-gpu-state experimental-nvidia-block battery-auto-switch battery-auto-switch-mode external-display-auto-switch save' -a 'external-display-auto-switch' -d 'Get or set automatic Hybrid fallback for dGPU-owned external displays'
complete -c cardwire -f -n '__fish_seen_subcommand_from config; and not __fish_seen_subcommand_from auto-apply-gpu-state experimental-nvidia-block battery-auto-switch battery-auto-switch-mode external-display-auto-switch save' -a 'save' -d 'Save current configuration to file'

# config <action> <value>
complete -c cardwire -f -n '__fish_seen_subcommand_from auto-apply-gpu-state; and test (count (commandline -opc)) -eq 3' -a "true\t'Enable' false\t'Disable'"
complete -c cardwire -f -n '__fish_seen_subcommand_from experimental-nvidia-block; and test (count (commandline -opc)) -eq 3' -a "true\t'Enable' false\t'Disable'"
complete -c cardwire -f -n '__fish_seen_subcommand_from battery-auto-switch; and test (count (commandline -opc)) -eq 3' -a "true\t'Enable' false\t'Disable'"
complete -c cardwire -f -n '__fish_seen_subcommand_from external-display-auto-switch; and test (count (commandline -opc)) -eq 3' -a "true\t'Enable' false\t'Disable'"
complete -c cardwire -f -n '__fish_seen_subcommand_from battery-auto-switch-mode; and test (count (commandline -opc)) -eq 3' -a "integrated\t'Integrated GPU only' hybrid\t'Hybrid mode' manual\t'Manual mode' smart\t'Smart mode'"

# manager <action>
complete -c cardwire -f -n '__fish_seen_subcommand_from manager; and not __fish_seen_subcommand_from status' -a 'status' -d 'Check if daemon is alive'

# debug <action>
complete -c cardwire -f -n '__fish_seen_subcommand_from debug; and not __fish_seen_subcommand_from diagnostic-gpu refresh-gpu' -a 'diagnostic-gpu' -d 'Run GPU diagnostics'
complete -c cardwire -f -n '__fish_seen_subcommand_from debug; and not __fish_seen_subcommand_from diagnostic-gpu refresh-gpu' -a 'refresh-gpu' -d 'Refresh GPU list in daemon'
"#;
