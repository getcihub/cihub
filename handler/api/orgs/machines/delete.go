package machines

import (
	"net/http"

	"github.com/getcihub/cihub/handler/api/render"
)

// HandleDelete returns an http.HandlerFunc that handles an
// http.Request to delete a machine entry from the datastore.
func HandleDelete() http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		render.NotImplemented(w)
	}
}
