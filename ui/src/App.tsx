import { RouterProvider } from "@tanstack/react-router";

import { ActionFeedbackProvider } from "view/components/experimental/ActionFeedback";
import { HaloProvider } from "view/components/experimental/Halo";

import { router } from "./router";

function App() {
	return (
		<ActionFeedbackProvider>
			<HaloProvider>
				<RouterProvider router={router} defaultPreload="intent" context={{}} />
			</HaloProvider>
		</ActionFeedbackProvider>
	);
}

export default App;
