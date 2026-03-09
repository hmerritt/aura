import * as stylex from "@stylexjs/stylex";
import { createFileRoute } from "@tanstack/react-router";

export const Route = createFileRoute("/")({
	component: IndexRoute
});

export function IndexRoute() {
	return (
		<div {...stylex.props(styles.page)}>
			<header data-testid="settings-header" {...stylex.props(styles.header)}>
				<div {...stylex.props(styles.brand)}>
					<img
						alt="aura logo"
						data-testid="settings-header-logo"
						src="/logo.png"
						{...stylex.props(styles.logo)}
					/>
					<h1 {...stylex.props(styles.title)}>aura</h1>
				</div>
				<p {...stylex.props(styles.version)}>Version 0.22.32</p>
			</header>
		</div>
	);
}

const styles = stylex.create({
	page: {
		width: "100%",
		minHeight: "100vh",
		backgroundColor: "#FFFFFF"
	},
	header: {
		width: "100%",
		height: "55px",
		paddingBlock: '15px',
		paddingInline: '24px',
		display: "flex",
		alignItems: "center",
		justifyContent: "space-between",
		backgroundColor: "#F2F2F2",
		borderBottomWidth: "1px",
		borderBottomStyle: "solid",
		borderBottomColor: "#DADADA"
	},
	brand: {
		display: "flex",
		alignItems: "center",
		gap: "15px"
	},
	logo: {
		width: "26px",
		height: "26px",
		objectFit: "contain",
		flexShrink: 0
	},
	title: {
		fontSize: "1.5rem",
		lineHeight: 1,
		fontWeight: 500,
		color: "#111111",
		textTransform: "lowercase"
	},
	version: {
		fontSize: "1.3rem",
		lineHeight: 1,
		fontWeight: 500,
		color: "#666666"
	}
});
