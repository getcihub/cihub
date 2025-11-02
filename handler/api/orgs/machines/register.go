package machines

import (
	"net/http"

	"github.com/getcihub/cihub/handler/api/render"
)

// HandleRegister returns an http.HandlerFunc that writes json-encoded
// machine information to the http response body with the machine token.
func HandleRegister() http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		render.NotImplemented(w)
	}
}
