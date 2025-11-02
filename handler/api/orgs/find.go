package orgs

import (
	"net/http"

	"github.com/getcihub/cihub/handler/api/render"
	"github.com/getcihub/cihub/handler/api/request"
)

// HandleFind returns an http.HandlerFunc that writes the
// json-encoded organization details to the response body.
func HandleFind() http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		ctx := r.Context()
		installation, _ := request.InstallationFrom(ctx)
		membership, _ := request.MembershipFrom(ctx)
		installation.Membership = membership
		render.OK(w, render.ReasonResolved, installation)
	}
}
