import * as stylex from "@stylexjs/stylex";
import { useEffect, useRef, useState } from "react";

import { isMobile } from "lib/device";
import { useEventListener } from "lib/hooks";

const DEFAULT_DURATION = 900;
const ENTER_DURATION = 120;
const EXIT_DURATION = 180;
const CURSOR_OFFSET = 16;

type ActionFeedbackPhase = "enter" | "exit";
export type ActionFeedbackAnimation = "fade";

export type TriggerActionFeedbackInput = {
    element: React.ReactNode;
    duration?: number;
    cursorOffset?: number;
    entranceAnimation?: ActionFeedbackAnimation;
    exitAnimation?: ActionFeedbackAnimation;
};

type ActionFeedbackPayload = {
    id: number;
    element: React.ReactNode;
    duration: number;
    cursorOffset: number;
    entranceAnimation: ActionFeedbackAnimation;
    exitAnimation: ActionFeedbackAnimation;
};

export type ActionFeedbackProviderProps = {
    children: React.ReactNode;
};

type TriggerActionFeedbackHandler = (payload: TriggerActionFeedbackInput) => void;

let triggerActionFeedbackHandler: TriggerActionFeedbackHandler | null = null;

const normalizeDuration = (duration?: number): number => {
    if (!Number.isFinite(duration) || (duration ?? 0) <= 0) {
        return DEFAULT_DURATION;
    }
    return Math.floor(duration as number);
};

const normalizePayload = (
    id: number,
    payload: TriggerActionFeedbackInput
): ActionFeedbackPayload => ({
    id,
    element: payload.element,
    duration: normalizeDuration(payload.duration),
    cursorOffset: payload.cursorOffset || CURSOR_OFFSET,
    entranceAnimation: payload.entranceAnimation ?? "fade",
    exitAnimation: payload.exitAnimation ?? "fade"
});

const clearTimeoutRef = (timeoutRef: React.MutableRefObject<number | undefined>) => {
    if (timeoutRef.current != null) {
        clearTimeout(timeoutRef.current);
        timeoutRef.current = undefined;
    }
};

export const triggerActionFeedback = (payload: TriggerActionFeedbackInput): void => {
    triggerActionFeedbackHandler?.(payload);
};

export const ActionFeedbackProvider = ({ children }: ActionFeedbackProviderProps) => {
    const [phase, setPhase] = useState<ActionFeedbackPhase>("exit");
    const [cursor, setCursor] = useState({ x: 0, y: 0 });
    const [feedback, setFeedback] = useState<ActionFeedbackPayload | null>(null);

    const currentId = useRef(0);
    const cursorRef = useRef({ x: 0, y: 0 });
    const feedbackRef = useRef<ActionFeedbackPayload | null>(null);
    const enterTimeout = useRef<number | undefined>(undefined);
    const durationTimeout = useRef<number | undefined>(undefined);
    const exitTimeout = useRef<number | undefined>(undefined);

    useEffect(() => {
        feedbackRef.current = feedback;
    }, [feedback]);

    useEventListener("mousemove", (evt) => {
        if (isMobile) return;

        const { x, y } = (evt as MouseEvent) || {};
        if (x == null || y == null) return;

        cursorRef.current = { x, y };
        if (feedbackRef.current) {
            setCursor({ x, y });
        }
    });

    useEffect(() => {
        if (isMobile) return;
        cursorRef.current = {
            x: Math.round(window.innerWidth / 2),
            y: Math.round(window.innerHeight / 2)
        };
        setCursor(cursorRef.current);
    }, []);

    useEffect(() => {
        triggerActionFeedbackHandler = (payload) => {
            if (isMobile) return;

            const id = currentId.current + 1;
            currentId.current = id;
            const normalized = normalizePayload(id, payload);

            clearTimeoutRef(enterTimeout);
            clearTimeoutRef(durationTimeout);
            clearTimeoutRef(exitTimeout);

            setCursor(cursorRef.current);
            setFeedback(normalized);
            setPhase("exit");

            enterTimeout.current = window.setTimeout(() => {
                if (currentId.current !== id) return;
                setPhase("enter");
            }, 0);

            durationTimeout.current = window.setTimeout(() => {
                if (currentId.current !== id) return;
                setPhase("exit");

                exitTimeout.current = window.setTimeout(() => {
                    if (currentId.current !== id) return;
                    setFeedback(null);
                }, EXIT_DURATION);
            }, normalized.duration);
        };

        return () => {
            if (triggerActionFeedbackHandler) {
                triggerActionFeedbackHandler = null;
            }
            clearTimeoutRef(enterTimeout);
            clearTimeoutRef(durationTimeout);
            clearTimeoutRef(exitTimeout);
        };
    }, []);

    if (isMobile || !feedback) {
        return children;
    }

    return (
        <>
            {children}
            <div
                {...stylex.props(
                    styles.layer,
                    styles.position(
                        cursor.x + feedback.cursorOffset,
                        cursor.y + feedback.cursorOffset
                    ),
                    phase === "enter"
                        ? styles.enter(feedback.entranceAnimation)
                        : styles.exit(feedback.exitAnimation)
                )}
                data-action-feedback
                data-action-feedback-phase={phase}
                data-action-feedback-x={cursor.x}
                data-action-feedback-y={cursor.y}
                data-testid="action-feedback-layer"
            >
                {feedback.element}
            </div>
        </>
    );
};

const styles = stylex.create({
    layer: {
        left: 0,
        opacity: 0,
        pointerEvents: "none",
        position: "fixed",
        top: 0,
        willChange: "transform, opacity",
        zIndex: 2147483647
    },
    position: (x: number, y: number) => ({
        transform: `translate3d(${x}px, ${y}px, 0)`
    }),
    enter: (_animation: ActionFeedbackAnimation) => ({
        opacity: 1,
        transitionDuration: `${ENTER_DURATION}ms`,
        transitionProperty: "opacity",
        transitionTimingFunction: "ease-out"
    }),
    exit: (_animation: ActionFeedbackAnimation) => ({
        opacity: 0,
        transitionDuration: `${EXIT_DURATION}ms`,
        transitionProperty: "opacity",
        transitionTimingFunction: "ease-in"
    })
});
