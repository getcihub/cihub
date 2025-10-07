package user

import (
	"net/http"

	"github.com/getcihub/cihub/handler/api/render"
	"github.com/getcihub/cihub/handler/api/request"
)

// HandleFind returns an http.HandlerFunc that writes json-encoded
// account information to the http response body.
func HandleFind() http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		ctx := r.Context()
		viewer, _ := request.UserFrom(ctx)
		render.OK(w, render.ReasonResolved, viewer)
	}
}
