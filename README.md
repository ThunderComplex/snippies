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
  -s, --snippie-dir <SNIPPIE_DIR>  Directory where snippie .md files reside
  -o, --out-dir <OUT_DIR>          Output directory, ready to serve
  -p <PORT>                        Port to listen on [default: 8192]
  -h, --help                       Print help
```

Create a local `.env` file in the project root to enable HTTP Basic Auth for
`POST /new`:

```env
SNIPPIES_NEW_SNIPPIE_USER=your-username
SNIPPIES_NEW_SNIPPIE_PASSWORD=your-password
```

The `/new` route will require credentials whenever both values are set. Put the
server behind HTTPS when hosting it publicly.

## Development notice  

This project is a heavy work-in-progress and still in early stages of development.  
This document might not reflect all of the current capabilities of this project.  
When in doubt, just read the source code.
