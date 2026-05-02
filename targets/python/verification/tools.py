from main_types import AuwgentTools


class Tools(AuwgentTools):

    async def get_location(self) -> str:
        return "Tarkwa"

    async def get_marks(self,id) -> str:
        return "A,B,C,D"
