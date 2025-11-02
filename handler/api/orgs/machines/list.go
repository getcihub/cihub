package machines

import (
	"net/http"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/render"
	"github.com/getcihub/cihub/handler/api/request"
	"github.com/getcihub/cihub/logger"
)

// HandleList returns an http.HandlerFunc that processes http
// requests to list all machines of an organization.
func HandleList(machines core.MachineStore) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		installation, _ := request.InstallationFrom(r.Context())

		// List machines for this organization
		results, err := machines.List(r.Context(), installation.Login)
		if err != nil {
			render.InternalError(w)
			logger.FromRequest(r).
				WithError(err).
				WithField("owner", installation.Login).
				Warnln("api: cannot list machines")
			return
		}

		render.OK(w, render.ReasonListed, results)
	}
}
