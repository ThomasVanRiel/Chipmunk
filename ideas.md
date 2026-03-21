# Ideas

## inbox

* CLI output to stdout for easy piping
* Add hooks in postprocessor to run a command using the postprocessed NC code (e.g. to send it to the machine)
* User can check for errors in postprocessing. The CAM program does not care (only impossible requests are errors, safety or machine limits is not our concern). Add a return option for the postprocessor to return a descriptive error (maybe from enum error types? e.g. overtravel)
