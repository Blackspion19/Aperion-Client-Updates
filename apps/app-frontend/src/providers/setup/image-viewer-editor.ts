import {
	type ImageViewerEditorData,
	type ImageViewerEditorSource,
	provideImageViewerEditor,
} from '@aperion/ui'
import { readFile } from '@tauri-apps/plugin-fs'

export function setupImageViewerEditorProvider() {
	provideImageViewerEditor({
		async loadEditorData(source: ImageViewerEditorSource): Promise<ImageViewerEditorData> {
			return {
				source: new Blob([await readFile(source.path)], { type: 'image/png' }),
			}
		},
	})
}
