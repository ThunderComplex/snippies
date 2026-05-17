package main

import (
	"encoding/base64"
	"encoding/json"
	"flag"
	"fmt"
	"io"
	"log"
	"net/http"
	"net/url"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"time"

	"github.com/gomarkdown/markdown"
	"github.com/gomarkdown/markdown/html"
	"github.com/gomarkdown/markdown/parser"

	"github.com/flosch/pongo2/v7"

	"github.com/joho/godotenv"

	"github.com/fsnotify/fsnotify"
)

var snippieTemplate = pongo2.Must(pongo2.FromFile("frontend/templates/snippie.html"))
var indexTemplate = pongo2.Must(pongo2.FromFile("frontend/templates/index.html"))
var errorTemplate = pongo2.Must(pongo2.FromFile("frontend/templates/error.html"))
var newTemplate = pongo2.Must(pongo2.FromFile("frontend/templates/new.html"))
var args Args
var themePresetNameRegex = regexp.MustCompile(`^[a-zA-Z0-9-_]{1,64}$`)
var themePresetColorRegex = regexp.MustCompile(`^#[0-9a-fA-F]{6}$`)

type Args struct {
	snippieDir string
	outDir     string
	port       int16
}

type ThemePreset struct {
	Name   string            `json:"name"`
	Colors map[string]string `json:"colors"`
}

func main() {
	err := godotenv.Load()

	if err != nil {
		log.Fatal(err)
	}

	var snippieDir, outDir string
	flag.StringVar(&snippieDir, "snippie-dir", "snippies", "Directory where snippie .md files reside")
	flag.StringVar(&outDir, "out-dir", "output", "Output directory, ready to serve")

	port := flag.Int("port", 8192, "Port to listen on")

	flag.Parse()

	log.Println("snippie dir: ", snippieDir)
	log.Println("output dir: ", outDir)
	log.Println("port: ", *port)

	args = Args{
		snippieDir: snippieDir,
		outDir:     outDir,
		port:       int16(*port),
	}

	watcher, err := fsnotify.NewWatcher()

	if err != nil {
		log.Fatal(err)
	}

	defer watcher.Close()

	err = watcher.Add(args.snippieDir)

	if err != nil {
		log.Fatal(err)
	}

	err = watcher.Add("frontend/templates")

	if err != nil {
		log.Fatal(err)
	}

	err = watcher.Add("frontend/static")

	if err != nil {
		log.Fatal(err)
	}

	go func() {
		shouldUpdate := true
		updateDebouncer := time.NewTicker(500 * time.Millisecond)

		for {
			select {
			case event, ok := <-watcher.Events:
				if !ok {
					return
				}

				if shouldUpdate && event.Has(fsnotify.Write) {
					shouldUpdate = false
					log.Println("rendering snippies")
					renderSnippies()

				}
			case err, ok := <-watcher.Errors:
				if !ok {
					return
				}

				log.Println("watcher error: ", err)
			case _ = <-updateDebouncer.C:
				shouldUpdate = true
			}
		}
	}()

	renderSnippies()

	http.HandleFunc("GET /", fileHandler("index.html"))
	http.HandleFunc("GET /new", fileHandler("new.html"))
	http.HandleFunc("GET /error", fileHandler("error.html"))
	http.HandleFunc("GET /static/", fileFromRequestHandler)
	http.HandleFunc("GET /snippie/", fileFromRequestHandler)
	http.HandleFunc("POST /api/new", apiNewHandler)
	http.HandleFunc("POST /api/theme-presets", apiThemePresetHandler)

	http.ListenAndServe(fmt.Sprintf(":%v", args.port), nil)
}

func handleAuth(w http.ResponseWriter, req *http.Request) bool {
	username := os.Getenv("SNIPPIES_NEW_SNIPPIE_USER")
	password := os.Getenv("SNIPPIES_NEW_SNIPPIE_PASSWORD")

	if username != "" && password != "" {
		expectedAuth := fmt.Sprintf("Basic %s", base64.StdEncoding.EncodeToString(fmt.Appendf(nil, "%s:%s", username, password)))
		authHeader := req.Header.Get("Authorization")

		if authHeader != expectedAuth {
			w.Header().Set("WWW-Authenticate", `Basic realm="snippies"`)
			w.WriteHeader(401)
			w.Write([]byte("Authentication required"))
			return false
		}
	}

	return true
}

func redirectTo(location string, w http.ResponseWriter) {
	w.Header().Set("Location", location)
	w.WriteHeader(303)
}

func validatePreset(preset *ThemePreset) bool {
	if !themePresetNameRegex.MatchString(preset.Name) {
		return false
	}

	var colorIds [9]string = [9]string{
		"c-bg",
		"c-text",
		"c-container",
		"c-list-item",
		"c-list-shadow",
		"c-primary",
		"c-text-highlight",
		"c-code-bg",
		"c-back-link",
	}

	var missingColors []string

	if len(preset.Colors) != len(colorIds) {
		return false
	}

	for _, colorId := range colorIds {
		presetColor, ok := preset.Colors[colorId]

		if !ok {
			log.Println("invalid color: ", colorId)
			missingColors = append(missingColors, colorId)
			continue
		}

		if !themePresetColorRegex.MatchString(presetColor) {
			log.Println("invalid color: ", presetColor)
			return false
		}
	}

	if len(missingColors) > 0 {
		log.Println("missing colors: ", missingColors)
		return false
	}

	return true
}

func apiThemePresetHandler(w http.ResponseWriter, req *http.Request) {
	if handleAuth(w, req) {
		defer req.Body.Close()
		var preset ThemePreset

		err := json.NewDecoder(req.Body).Decode(&preset)

		if err != nil {
			log.Println("preset decoding error: ", err)

			redirectTo("/error", w)
			return
		}

		if !validatePreset(&preset) {
			redirectTo("/error", w)
			return
		}

		jsonData, err := json.Marshal(preset)

		if err != nil {
			redirectTo("/error", w)
			return
		}

		jsonData = append(jsonData, '\n')

		presetFile, err := os.OpenFile("frontend/static/theme-presets.jsonl", os.O_APPEND|os.O_WRONLY, 0644)

		if err != nil {
			redirectTo("/error", w)
			return
		}

		defer presetFile.Close()

		_, err = presetFile.Write(jsonData)

		if err != nil {
			redirectTo("/error", w)
			return
		}

		// Wait for snippies to be rebuilt, so we don't accidentally panic
		time.Sleep(100 * time.Millisecond)
	}

	redirectTo("/", w)
}

func apiNewHandler(w http.ResponseWriter, req *http.Request) {
	if handleAuth(w, req) {
		defer req.Body.Close()
		req.ParseForm()

		if len(req.Form["title"]) == 1 && len(req.Form["contents"]) == 1 {
			title := req.Form["title"][0]
			contents := req.Form["contents"][0]

			path := filepath.Join(args.snippieDir, title) + ".md"
			err := os.WriteFile(path, []byte(contents), 0644)

			if err != nil {
				log.Println("could not create new snippie: ", err)

				redirectTo("/error", w)
				return
			}

			// Wait for snippies to be rebuilt, so we don't accidentally panic
			time.Sleep(100 * time.Millisecond)
		}
	}

	redirectTo("/", w)
}

// TODO: Don't read file to write it, or cache it maybe?
func fileHandler(file string) func(http.ResponseWriter, *http.Request) {
	return func(w http.ResponseWriter, req *http.Request) {
		path := filepath.Join(args.outDir, file)

		http.ServeFile(w, req, path)
	}
}

func fileFromRequestHandler(w http.ResponseWriter, req *http.Request) {
	requestUri, err := url.PathUnescape(req.RequestURI)

	if err != nil {
		log.Fatal(err)
	}

	acceptHeader := req.Header["Accept"][0]
	filePath := filepath.Join(args.outDir, requestUri)
	content, err := os.ReadFile(filePath)

	if err != nil {
		log.Fatal(err)
	}

	if strings.HasSuffix(filePath, ".js") {
		w.Header().Set("Content-Type", "application/javascript")
	}

	if strings.Contains(acceptHeader, "text/css") {
		w.Header().Set("Content-Type", "text/css")
	}

	w.Write(content)
}

func renderSnippies() {
	workDir, err := os.Getwd()

	if err != nil {
		log.Fatal(err)
	}

	snippiePath := filepath.Join(workDir, args.snippieDir)
	outPath := filepath.Join(workDir, args.outDir)
	outSnippiesPath := filepath.Join(outPath, "snippie")
	outStaticPath := filepath.Join(outPath, "static")

	snippieEntries, err := os.ReadDir(snippiePath)

	if err != nil {
		log.Fatal(err)
	}

	err = os.RemoveAll(outPath)

	if err != nil {
		log.Fatal(err)
	}

	err = os.MkdirAll(outSnippiesPath, 0755)

	if err != nil {
		log.Fatal(err)
	}

	err = os.MkdirAll(outStaticPath, 0755)

	if err != nil {
		log.Fatal(err)
	}

	var snippieNames []string

	for _, snippieEntry := range snippieEntries {
		if snippieEntry.IsDir() {
			continue
		}

		snippieFilePath := filepath.Join(snippiePath, snippieEntry.Name())

		snippieContents, snippieErr := os.ReadFile(snippieFilePath)

		if snippieErr != nil {
			log.Fatal(snippieErr)
		}

		snippieHtml := markdownToHtml(snippieContents)
		snippieRendered := renderSnippieTemplate(snippieHtml)
		snippieExt := filepath.Ext(snippieEntry.Name())

		outFilePath := filepath.Join(outSnippiesPath, snippieEntry.Name())
		outFilePath = strings.TrimSuffix(outFilePath, snippieExt)
		outFilePath += ".html"

		writeErr := os.WriteFile(outFilePath, []byte(snippieRendered), 0644)

		if writeErr != nil {
			log.Fatal(writeErr)
		}

		snippieNames = append(snippieNames, strings.TrimSuffix(snippieEntry.Name(), snippieExt))
	}

	createStaticTemplates(snippieNames, outPath)
	copyStaticFiles(outStaticPath)
}

func copyStaticFiles(path string) {
	files := []struct {
		src string
		dst string
	}{
		{"frontend/static/app.css", filepath.Join(path, "app.css")},
		{"frontend/static/theme.js", filepath.Join(path, "theme.js")},
		{"frontend/static/theme-presets.jsonl", filepath.Join(path, "theme-presets.jsonl")},
		{"frontend/static/prism.css", filepath.Join(path, "prism.css")},
		{"frontend/static/prism.js", filepath.Join(path, "prism.js")},
		{"frontend/static/favicon.ico", filepath.Join(path, "favicon.ico")},
	}

	for _, item := range files {
		err := copyFile(item.src, item.dst)

		if err != nil {
			log.Fatal(err)
		}
	}
}

func createStaticTemplates(snippieNames []string, outPath string) {
	ctx := pongo2.Context{
		"title":    "Snippie",
		"snippies": snippieNames,
	}

	indexContent, indexErr := indexTemplate.Execute(ctx)
	errorContent, errorErr := errorTemplate.Execute(ctx)
	newContent, newErr := newTemplate.Execute(ctx)

	if indexErr != nil || errorErr != nil || newErr != nil {
		log.Fatal(indexErr, errorErr, newErr)
	}

	writeErr1 := os.WriteFile(filepath.Join(outPath, "index.html"), []byte(indexContent), 0644)
	writeErr2 := os.WriteFile(filepath.Join(outPath, "new.html"), []byte(newContent), 0644)
	writeErr3 := os.WriteFile(filepath.Join(outPath, "error.html"), []byte(errorContent), 0644)

	if writeErr1 != nil || writeErr2 != nil || writeErr3 != nil {
		log.Fatal(writeErr1, writeErr2, writeErr3)
	}
}

func renderSnippieTemplate(template []byte) string {
	ctx := pongo2.Context{
		"title":   "Snippie",
		"content": string(template),
	}

	result, err := snippieTemplate.Execute(ctx)

	if err != nil {
		log.Fatal(err)
	}

	return result
}

func markdownToHtml(md []byte) []byte {
	extensions := parser.CommonExtensions | parser.AutoHeadingIDs | parser.NoEmptyLineBeforeBlock
	p := parser.NewWithExtensions(extensions)
	document := p.Parse(md)

	htmlFlags := html.CommonFlags | html.HrefTargetBlank
	opts := html.RendererOptions{Flags: htmlFlags}
	renderer := html.NewRenderer(opts)

	return markdown.Render(document, renderer)
}

func copyFile(src, dst string) error {
	in, err := os.Open(src)
	if err != nil {
		return err
	}
	defer in.Close()

	out, err := os.Create(dst)
	if err != nil {
		return err
	}
	defer out.Close()

	_, err = io.Copy(out, in)
	if err != nil {
		return err
	}

	return out.Sync()
}
