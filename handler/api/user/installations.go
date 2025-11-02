package user

import (
	"net/http"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/render"
	"github.com/getcihub/cihub/handler/api/request"
	"github.com/getcihub/cihub/logger"
)

// HandleInstallations returns an http.HandlerFunc that write a json-encoded
// list of installations to the response body.
func HandleInstallations(installations core.InstallationStore) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		viewer, _ := request.UserFrom(r.Context())

		result, err := installations.List(r.Context(), viewer)
		if err != nil {
			render.InternalErrorf(w, err.Error())
			logger.FromRequest(r).WithError(err).
				Debugln("api: cannot list repositories")
		} else {
			render.OK(w, render.ReasonListed, result)
		}
	}
}
