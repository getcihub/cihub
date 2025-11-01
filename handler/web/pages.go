package web

import (
	"crypto/md5"
	"fmt"
	"net/http"
	"time"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/web/dist"
)

func HandleIndex(session core.Session) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		user, _ := session.Get(r)
		if user == nil && r.URL.Path == "/" {

			http.Redirect(w, r, "/login", http.StatusSeeOther)
			return
		}

		out := dist.MustLookup("/index.html")
		w.Header().Set("Content-Type", "text/html; charset=UTF-8")
		w.Write(out)
	}
}

func setupCache(h http.Handler) http.Handler {
	data := []byte(time.Now().String())
	etag := fmt.Sprintf("%x", md5.Sum(data))

	return http.HandlerFunc(
		func(w http.ResponseWriter, r *http.Request) {
			w.Header().Set("Cache-Control", "public, max-age=31536000")
			w.Header().Del("Expires")
			w.Header().Del("Pragma")
			w.Header().Set("ETag", etag)
			h.ServeHTTP(w, r)
		},
	)
}
