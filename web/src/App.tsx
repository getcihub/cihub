import { Outlet } from '@tanstack/react-router';

function App() {
    return (
        <div className='flex-1 flex flex-col'>
            <Outlet />
        </div>
    );
}

export default App;
