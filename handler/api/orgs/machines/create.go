package machines

import (
	"encoding/json"
	"net/http"
	"time"

	"github.com/dchest/uniuri"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/render"
	"github.com/getcihub/cihub/handler/api/request"
	"github.com/getcihub/cihub/logger"
)

type createInput struct {
	Name   string   `json:"name"`
	Arch   string   `json:"arch"`
	CPU    int64    `json:"cpu"`
	Labels []string `json:"labels,omitempty"`
	RAM    int64    `json:"ram"`
}

type machineWithToken struct {
	*core.Machine
	Token string `json:"token"`
}

// HandleCreate returns an http.HandlerFunc to create
// a new machine in the system.
func HandleCreate(machines core.MachineStore) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		installation, _ := request.InstallationFrom(r.Context())

		in := new(createInput)
		err := json.NewDecoder(r.Body).Decode(in)
		if err != nil {
			render.BadRequest(w)
			logger.FromRequest(r).WithError(err).
				Debugln("api: cannot unmarshal request body")
			return
		}

		machine := &core.Machine{
			Name:    in.Name,
			Owner:   installation.Login,
			Arch:    in.Arch,
			CPU:     in.CPU,
			RAM:     in.RAM,
			Status:  core.MachineStatusOffline,
			Created: time.Now().Unix(),
			Updated: time.Now().Unix(),
			Token:   uniuri.NewLen(32),
		}

		err = machines.Create(r.Context(), machine)
		if err != nil {
			render.InternalErrorf(w, err.Error())
			logger.FromRequest(r).WithError(err).
				Errorln("api: cannot create machine")
			return
		}

		render.OK(w, render.ReasonCreated, &machineWithToken{
			Machine: machine,
			Token:   machine.Token,
		})
	}
}
