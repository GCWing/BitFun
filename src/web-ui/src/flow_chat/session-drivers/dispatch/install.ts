/**
 * Boot-time installation of the dispatch driver's background machinery.
 * FlowChatManager installs this once; the observer itself lives in
 * features/dispatch and stays private to the dispatch driver.
 */

export { installDispatchJobObserver } from '@/features/dispatch/DispatchJobObserver';
