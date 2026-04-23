from main_types import AuwgentTools, Person


class Tools(AuwgentTools):

    async def get_location(self) -> str:
        return "Tarkwa"

    async def get_user_name_age(self) -> "Person":
        return Person(name="Theophlilus", age=99)
