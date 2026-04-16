declare module "pdfjs-dist" {
  export function getDocument(params: {
    data?: Uint8Array;
    url?: string;
  }): {
    promise: Promise<PDFDocumentProxy>;
  };

  export interface PDFDocumentProxy {
    numPages: number;
    getPage(pageNumber: number): Promise<PDFPageProxy>;
    destroy(): void;
  }

  export interface PDFPageProxy {
    getViewport(params: { scale: number }): PageViewport;
    render(params: {
      canvasContext: CanvasRenderingContext2D;
      viewport: PageViewport;
    }): { promise: Promise<void> };
  }

  export interface PageViewport {
    width: number;
    height: number;
  }

  export const version: string;

  export const GlobalWorkerOptions: {
    workerSrc: string;
  };
}
