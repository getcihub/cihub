package machines

import (
	"encoding/json"
	"net/http"

	"github.com/go-chi/chi/v5"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/render"
	"github.com/getcihub/cihub/logger"
)

type machineUpdate struct {
	Status *string `json:"status,omitempty"`
}

// HandleUpdate returns an http.HandlerFunc that processes http
// requests to update a machine.
func HandleUpdate(machines core.MachineStore) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		var (
			owner = chi.URLParam(r, "owner")
			name  = chi.URLParam(r, "name")
		)

		in := new(machineUpdate)
		err := json.NewDecoder(r.Body).Decode(in)
		if err != nil {
			render.BadRequestWithReason(w, err.Error())
			return
		}

		machine, err := machines.Find(r.Context(), owner, name)
		if err != nil {
			render.NotFound(w)
			logger.FromRequest(r).
				WithError(err).
				WithField("owner", owner).
				WithField("name", name).
				Debugln("api: machine not found")
			return
		}

		if in.Status != nil {
			switch *in.Status {
			case core.MachineStatusOnline, core.MachineStatusPaused:
				machine.Status = *in.Status
			default:
				render.BadRequestWithReason(w, "invalid status value")
				return
			}
		}

		err = machines.Update(r.Context(), machine)
		if err != nil {
			render.InternalErrorf(w, err.Error())
			logger.FromRequest(r).
				WithError(err).
				WithField("owner", owner).
				WithField("name", name).
				Errorln("api: cannot update machine")
			return
		}

		render.OK(w, render.ReasonUpdated, machine)
	}
}
