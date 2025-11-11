package api

import (
	"net/http"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/handler/api/render"
	"github.com/getcihub/cihub/version"
)

type varz struct {
	App     *githubAppInfo `json:"github"`
	Version *versionInfo   `json:"version"`
}

type githubAppInfo struct {
	Name   string `json:"name"`
	Server string `json:"server"`
}

type versionInfo struct {
	Source  string `json:"source,omitempty"`
	Version string `json:"version,omitempty"`
	Commit  string `json:"commit,omitempty"`
}

// HandleVarz creates an http.HandlerFunc that
// exposes internal system information.
func HandleVarz(system *core.System) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		v := &varz{
			App: &githubAppInfo{
				Name:   system.AppName,
				Server: system.Server,
			},
			Version: &versionInfo{
				Source:  version.GitRepository,
				Commit:  version.GitCommit,
				Version: version.Version.String(),
			},
		}

		render.OK(w, render.ReasonResolved, v)
	}
}
