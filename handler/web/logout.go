package web

import (
	"net/http"

	"github.com/getcihub/cihub/core"
)

// HandleLogout creates an http.HandlerFunc that handles
// session termination.
func HandleLogout(session core.Session) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		session.Delete(w)
		http.Redirect(w, r, "/login", http.StatusSeeOther)
	}
}
