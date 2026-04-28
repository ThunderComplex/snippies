# Snippies

Self-hostable code snippet solution

## What is snippies?

Snippies is a minimal solution to create your own self-hosted repository of code snippets.  
A snippie is just a markdown file that will be rendered as a HTML file. A overview of all snippies
will be generated as well.  
Put all of your code snippets as separate .md files into a directory (no subdirectories!) and run the tool.  
The generated output folder will contain all the necessary assets to statically serve the directory as-is.  
Under the hood snippies uses prism.js for syntax highlighting.

## Usage  

```
snippies [OPTIONS] --snippie-dir <SNIPPIE_DIR>

Options:
  -s, --snippie <SNIPPIE>
          Directory where snippie .md files reside
  -o, --out <OUT>
          Output directory, ready to serve
  -c, --clear-output
          Delete output directory contents before writing new files
  -p <PORT>
          Port to listen on [default: 8192]
      --watch
          Watch for file changes (not needed when 'dev' is enabled)
      --serve
          Start a server and watch for file changes
      --new-snippie-user <NEW_SNIPPIE_USER>
          Username required to create new snippies
      --new-snippie-password <NEW_SNIPPIE_PASSWORD>
          Password required to create new snippies
  -h, --help
          Print help
```

You can also set `SNIPPIES_NEW_SNIPPIE_USER` and `SNIPPIES_NEW_SNIPPIE_PASSWORD`
instead of passing the credentials on the command line. The `/new` route will
require HTTP Basic Auth whenever credentials are configured.
Put the server behind HTTPS when hosting it publicly.

## Development notice  

This project is a heavy work-in-progress and still in early stages of development.  
This document might not reflect all of the current capabilities of this project.  
When in doubt, just read the source code.
