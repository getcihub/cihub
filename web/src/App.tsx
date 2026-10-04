export function App({ path }: { path: string }) {
  return (
    <>
      <header className="header">
        <a href="/">CIHub</a>
      </header>
      <main className="main">{path === "/" ? <Home /> : <NotFound />}</main>
    </>
  );
}

function Home() {
  return (
    <>
      <h1>Merge queue</h1>
      <p>The dashboard of this instance.</p>
    </>
  );
}

function NotFound() {
  return (
    <>
      <h1>Page not found</h1>
      <p>
        <a href="/">Back to the home page</a>
      </p>
    </>
  );
}
